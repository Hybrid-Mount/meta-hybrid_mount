// SPDX-License-Identifier: GPL-3.0-only

//! Userspace backend for Hybrid Mount's own VFS kernel subsystem, the `hybridmount` module.

pub mod backend;
pub mod doctor;
pub mod exec;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod lkm;
pub mod lkm_target;
pub mod protocol;
pub mod rule;
pub mod sys;

#[cfg(test)]
#[path = "test_support.rs"]
mod test_support;

/// Whether the VFS backend can be offered on this device right now.
///
/// Every surface that advertises VFS reads this, so a kernel without the module shows no VFS
/// option, count or description line anywhere.
pub fn available() -> bool {
    doctor::responds_now()
}

/// Brings the provider up before the mount plan is built, and reports whether it is usable.
///
/// The plan degrades every `vfs` rule to `ignore` while [`available`] is false, and the executor
/// only reaches its own load step for a plan that still contains vfs work. Loading here therefore
/// has to happen *before* planning: probing alone can never make the first plan contain vfs, so a
/// device whose kernel does not carry the module would otherwise never load the bundled one.
///
/// `available` is the read-only probe and `load` performs the `insmod`; both are injected so the
/// ordering and the no-load-when-unwanted rule are testable without a device. A load failure is
/// not an error here — the plan then degrades exactly as it does when no module is bundled.
pub fn ensure_loaded_for_plan(
    wants_vfs: bool,
    available: impl Fn() -> bool,
    load: impl FnOnce() -> crate::errors::Result<()>,
) -> bool {
    if !wants_vfs || available() {
        return available();
    }

    log::info!("vfs is configured but the key type does not answer; loading the bundled module");
    if let Err(err) = load() {
        log::warn!("bundled vfs module load failed: {err}");
    }

    let ready = available();
    if !ready {
        log::warn!("no vfs kernel provider after loading; vfs rules degrade to ignore");
    }
    ready
}

/// Whether an unavailable provider has to abort the boot instead of degrading.
///
/// `vfs_strict` is a promise about a backend the config actually uses, so it applies only when a
/// rule selects vfs: a device configured for overlay or magic must still boot normally with the
/// option left on. The boot pipeline asks this before planning, because the plan rewrites every
/// `vfs` rule to `ignore` and the executor then returns early without reaching its own strict
/// checks, which would otherwise make the option silently ineffective.
pub const fn unavailable_is_fatal(wants_vfs: bool, strict: bool, available: bool) -> bool {
    wants_vfs && strict && !available
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// The regression this guards: a probe-only plan gate makes the load step unreachable, so
    /// a device whose kernel lacks hybridmount never loads the module bundled for exactly that
    /// case. Asking for vfs must reach the loader.
    #[test]
    fn a_wanted_vfs_loads_the_bundled_module_when_the_probe_is_silent() {
        let available = Cell::new(false);
        let loads = Cell::new(0);

        let ready = ensure_loaded_for_plan(
            true,
            || available.get(),
            || {
                loads.set(loads.get() + 1);
                available.set(true);
                Ok(())
            },
        );

        assert!(ready, "a successful load must leave vfs available");
        assert_eq!(loads.get(), 1, "the bundled module must be loaded once");
    }

    /// A kernel that already carries the module must not be asked to load anything.
    #[test]
    fn an_answering_provider_is_never_reloaded() {
        let loads = Cell::new(0);

        let ready = ensure_loaded_for_plan(
            true,
            || true,
            || {
                loads.set(loads.get() + 1);
                Ok(())
            },
        );

        assert!(ready);
        assert_eq!(loads.get(), 0, "an answering key type needs no insmod");
    }

    /// Overlay or magic configurations must not turn into an `insmod` on every boot.
    #[test]
    fn vfs_is_not_loaded_when_no_rule_asks_for_it() {
        let loads = Cell::new(0);

        let ready = ensure_loaded_for_plan(
            false,
            || false,
            || {
                loads.set(loads.get() + 1);
                Ok(())
            },
        );

        assert!(!ready);
        assert_eq!(loads.get(), 0, "an unused backend must not be loaded");
    }

    /// A failed load degrades instead of aborting the boot, which is what `vfs_strict` is for.
    #[test]
    fn a_failed_load_reports_unavailable_without_aborting() {
        let ready = ensure_loaded_for_plan(false, || false, || Ok(()));
        assert!(!ready);

        let ready = ensure_loaded_for_plan(
            true,
            || false,
            || {
                Err(crate::errors::Error::msg(
                    "no bundled module for this kernel",
                ))
            },
        );
        assert!(
            !ready,
            "a failed load must degrade rather than claim success"
        );
    }

    /// `vfs_strict` must actually fail the boot when a requested backend is missing. The plan
    /// rewrites vfs to `ignore` before the executor runs, so this predicate is the only guard.
    #[test]
    fn strict_fails_the_boot_when_a_requested_provider_is_missing() {
        assert!(unavailable_is_fatal(true, true, false));
    }

    /// The option is about a backend the config uses: leaving it on must not break a device
    /// that never selects vfs, nor one whose provider is present.
    #[test]
    fn strict_is_inert_without_a_vfs_rule_or_without_a_problem() {
        assert!(!unavailable_is_fatal(false, true, false), "no vfs rule");
        assert!(!unavailable_is_fatal(true, true, true), "provider present");
        assert!(!unavailable_is_fatal(true, false, false), "strict off");
    }

    /// The deadlock this module guards against is an ordering property of the boot pipeline, and
    /// reordering it is a one-line edit that no other test would notice: moving the load back
    /// below `build_plan` silently restores the unreachable-load bug while every behaviour unit
    /// test above still passes. This pins the order and the value the plan is handed.
    #[test]
    fn the_boot_pipeline_loads_before_it_plans() {
        let pipeline = include_str!("../pipeline.rs");

        let load = pipeline
            .find("ensure_loaded_for_plan")
            .expect("the boot pipeline must bring the provider up");
        let plan = pipeline
            .find("build_plan(")
            .expect("the boot pipeline must build a plan");
        assert!(
            load < plan,
            "the bundled module must be loaded before planning: the plan degrades vfs while the \
             key type is silent, so a load after it can never be reached"
        );

        // The plan has to receive that decision instead of re-probing, which would discard the
        // module just loaded and degrade every vfs rule anyway.
        let planned = &pipeline[plan..];
        let input = &planned[..planned.find("})").expect("the PlanInput call must close")];
        assert!(
            input.contains("vfs_available,"),
            "build_plan must be handed the post-load probe result"
        );
        assert!(
            !input.contains("crate::vfs::available()"),
            "build_plan must not re-probe, which would ignore the module just loaded"
        );
    }
}
