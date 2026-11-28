use std::collections::BTreeSet;
use std::path::Path;

use super::rules::{self, SavedRule};
use super::{device, ledger, policy};
use crate::errors::{Error, Result};
use crate::sys::transaction::MountTransaction;
use crate::vfs::backend::VfsKernel;
use crate::vfs::boot_guard::{VfsMutationSource, arm_for_mutation};

pub fn apply(
    source: &Path,
    target: &str,
    provider: &str,
    uids: &[u32],
    transaction: &mut MountTransaction<'_>,
) -> Result<()> {
    let mut saved = device::load()?;
    if saved
        .pending_rules
        .iter()
        .any(|rule| policy::paths_overlap(&rule.virtual_path, target))
    {
        return Err(Error::msg(format!(
            "Lua VFS target overlaps a planned rule: {target}"
        )));
    }
    let rule = SavedRule {
        module_id: rules::LUA_OWNER.into(),
        virtual_path: target.into(),
        real_path: source
            .to_str()
            .ok_or_else(|| Error::msg("VFS source requires UTF-8"))?
            .into(),
        flags: 0,
        uid: 0,
    };
    let mut kernel = device::Kernel::inspect()?;
    if kernel.0.provider().as_str() != provider {
        return Err(Error::msg("VFS provider changed after Lua mount planning"));
    }
    let batch = std::slice::from_ref(&rule);
    rules::preflight(&mut kernel, &[], batch)?;
    let _guard = arm_for_mutation(VfsMutationSource::Boot)?;
    let existing = kernel.uids()?;
    let added: Vec<_> = uids
        .iter()
        .copied()
        .filter(|uid| !existing.contains(uid))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    saved.vfs_provider = Some(provider.into());
    saved.pending_rules.push(rule.clone());
    ledger::save(&saved)?;
    if !added.is_empty() {
        let mut uid_kernel = device::Kernel::open(&saved)?;
        let rollback = added.clone();
        transaction
            .register_rollback_only("lua_vfs_uids", move || uid_kernel.remove_uids(&rollback));
        kernel.0.add_uids(&added)?;
    }
    rules::reconcile(&mut kernel, &[], batch)?;
    transaction.register_rollback_only(format!("lua_vfs:{target}"), move || {
        rules::reconcile(&mut kernel, &[rule], &[])
    });
    Ok(())
}
