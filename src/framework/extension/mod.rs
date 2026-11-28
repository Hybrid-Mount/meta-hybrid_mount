mod api;
mod core;
mod header;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

pub use self::api::{HookMount, HookPhase, ModuleEdits, MountKind};
use self::api::{v1, v2};
use crate::errors::{Error, Result};
use crate::mount::model::ModuleId;
use crate::mount::model::PartitionLayout;

#[derive(Default)]
pub struct Extensions {
    global: Vec<v1::Hooks>,
    modules: BTreeMap<ModuleId, Vec<v2::Hook>>,
}

impl Extensions {
    pub fn commands(&self, phase: HookPhase) -> Result<Vec<HookMount>> {
        let mut commands = Vec::new();
        for script in &self.global {
            script.collect(phase, &mut commands)?;
        }
        Ok(commands)
    }

    pub fn module_edits(&self, id: &ModuleId, layout: &PartitionLayout) -> Result<ModuleEdits> {
        let mut edits = ModuleEdits::default();
        if let Some(scripts) = self.modules.get(id) {
            for script in scripts {
                script.collect(id, layout, &mut edits)?;
                if edits.skip {
                    break;
                }
            }
        }
        for (target, source) in &edits.replacements {
            if !edits.excludes(target)
                && target.components().count() == layout.root_len(&target.to_string_lossy())
                && !source
                    .symlink_metadata()
                    .is_ok_and(|metadata| metadata.is_dir())
            {
                return Err(Error::msg(format!(
                    "module {id}: replacing partition {} requires a directory source, got {}",
                    target.display(),
                    source.display()
                )));
            }
        }
        Ok(edits)
    }
}
