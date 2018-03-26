use std::collections::HashMap;

use syn::{Ident, Path};
use syntax::check::{self, Idle, Init};
use syntax::{self, Resources, Statics};

use syntax::error::*;

pub struct App {
    pub device: Path,
    pub idle: Idle,
    pub init: Init,
    pub resources: Statics,
    pub tasks: Tasks,
}

pub type Tasks = HashMap<Ident, Task>;

#[allow(non_camel_case_types)]
pub enum Exception {
    PENDSV,
    SVCALL,
    SYS_TICK,
}

impl Exception {
    pub fn from(s: &str) -> Option<Self> {
        Some(match s {
            "PENDSV" => Exception::PENDSV,
            "SVCALL" => Exception::SVCALL,
            "SYS_TICK" => Exception::SYS_TICK,
            _ => return None,
        })
    }

    pub fn nr(&self) -> usize {
        match *self {
            Exception::PENDSV => 14,
            Exception::SVCALL => 11,
            Exception::SYS_TICK => 15,
        }
    }
}

pub enum Kind {
    Exception(Exception),
    Interrupt { enabled: bool },
}

pub struct Task {
    pub kind: Kind,
    pub path: Path,
    pub priority: u8,
    pub resources: Resources,
}

pub fn app(app: check::App) -> Result<App> {
    println!("-- checking tasks --");
    let app = App {
        device: app.device,
        idle: app.idle,
        init: app.init,
        resources: app.resources,
        tasks: app.tasks
            .into_iter()
            .map(|(k, v)| {
                let v =
                    ::check::task(k.as_ref(), v).chain_err(|| format!("checking task `{}`", k))?;

                Ok((k, v))
            })
            .collect::<Result<_>>()?,
    };

    println!("-- checking resources --");
    ::check::resources(&app).chain_err(|| "checking `resources`")?;

    Ok(app)
}

fn resources(app: &App) -> Result<()> {
    for name in &app.init.resources {
        if let Some(resource) = app.resources.get(name) {
            ensure!(
                resource.expr.is_some(),
                "resource `{}`, allocated to `init`, must have an initial value",
                name
            );
        } else {
            bail!(
                "resource `{}`, allocated to `init`, must be a data resource",
                name
            );
        }

        ensure!(
            !app.idle.resources.contains(name),
            "resources assigned to `init` can't be shared with `idle`"
        );

        ensure!(
            app.tasks
                .iter()
                .all(|(_, task)| !task.resources.contains(name)),
            "resources assigned to `init` can't be shared with tasks"
        )
    }

    for resource in app.resources.keys() {
        if app.init.resources.contains(resource) {
            continue;
        }

        if app.idle.resources.contains(resource) {
            continue;
        }

        if app.tasks
            .values()
            .any(|task| task.resources.contains(resource))
        {
            continue;
        }

        bail!("resource `{}` is unused", resource);
    }

    for (name, task) in &app.tasks {
        for resource in &task.resources {
            ensure!(
                app.resources.contains_key(&resource),
                "task {} contains an undeclared resource with name {}",
                name,
                resource
            );
        }
    }

    Ok(())
}

fn task(name: &str, task: syntax::check::Task) -> Result<Task> {
    let kind = match Exception::from(name) {
        Some(e) => {
            ensure!(
                task.enabled.is_none(),
                "`enabled` field is not valid for exceptions"
            );

            Kind::Exception(e)
        }
        None => {
            if task.enabled == Some(true) {
                bail!(
                    "`enabled: true` is the default value; this line can be \
                     omitted"
                );
            }

            Kind::Interrupt {
                enabled: task.enabled.unwrap_or(true),
            }
        }
    };

    Ok(Task {
        kind,
        path: task.path.ok_or("`path` field is missing")?,
        priority: task.priority.unwrap_or(1),
        resources: task.resources,
    })
}
