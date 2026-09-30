// SPDX-License-Identifier: GPL-3.0-only

//! KernelSU registration ownership and the try-umount wire protocol.

use std::collections::BTreeSet;
use std::ffi::CString;
use std::io;

// KernelSU uapi/supercall.h: __aligned_u64 arg; __u32 flags; __u8 mode.
#[repr(C, align(8))]
pub(crate) struct UmountCommand {
    pub arg: u64,
    pub flags: u32,
    pub mode: u8,
}

fn issue(
    path: &str,
    mode: u8,
    send: &mut impl FnMut(&UmountCommand) -> io::Result<()>,
) -> io::Result<()> {
    let path = CString::new(path)?;
    send(&UmountCommand {
        arg: path.as_ptr() as u64,
        flags: 2, // MNT_DETACH
        mode,
    })
}

pub(crate) fn release(
    paths: &[String],
    send: &mut impl FnMut(&UmountCommand) -> io::Result<()>,
) -> io::Result<()> {
    for path in paths {
        // Mode 0 wipes every writer's list; only mode 2 deletes this exact path.
        issue(path, 2, send)?;
    }
    Ok(())
}

#[derive(Default)]
pub(crate) struct Registrations {
    queued: BTreeSet<String>,
    installed: BTreeSet<String>,
}

impl Registrations {
    pub fn queue(&mut self, path: &str) {
        self.queued.insert(path.to_owned());
    }

    pub fn committed(&self) -> Vec<String> {
        self.installed.iter().cloned().collect()
    }

    pub fn commit(
        &mut self,
        send: &mut impl FnMut(&UmountCommand) -> io::Result<()>,
    ) -> io::Result<()> {
        for path in &self.queued {
            if !self.installed.contains(path) {
                issue(path, 1, send)?;
                self.installed.insert(path.clone());
            }
        }
        Ok(())
    }

    pub fn withdraw(
        &mut self,
        path: &str,
        send: &mut impl FnMut(&UmountCommand) -> io::Result<()>,
    ) -> io::Result<()> {
        self.queued.remove(path);
        if self.installed.contains(path) {
            release(&[path.to_owned()], send)?;
            self.installed.remove(path);
        }
        Ok(())
    }

    pub fn rollback(
        &mut self,
        send: &mut impl FnMut(&UmountCommand) -> io::Result<()>,
    ) -> io::Result<()> {
        self.queued.clear();
        for path in self.committed() {
            self.withdraw(&path, send)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::ffi::CStr;

    #[derive(Default)]
    struct Kernel {
        paths: BTreeSet<String>,
        fail: Option<String>,
        calls: usize,
    }

    impl Kernel {
        fn apply(&mut self, command: &UmountCommand) -> std::io::Result<()> {
            self.calls += 1;
            // SAFETY: issue() keeps the path's CString alive throughout the call.
            let path = unsafe { CStr::from_ptr(command.arg as *const std::ffi::c_char) }
                .to_str()
                .unwrap()
                .to_owned();
            if self.fail.as_deref() == Some(path.as_str()) {
                return Err(std::io::Error::other("injected ioctl failure"));
            }
            match command.mode {
                0 => self.paths.clear(),
                1 => {
                    assert_eq!(command.flags, 2, "register with MNT_DETACH");
                    if !self.paths.insert(path) {
                        return Err(std::io::ErrorKind::AlreadyExists.into());
                    }
                }
                2 => {
                    self.paths.remove(&path);
                }
                _ => panic!("invalid kernel operation"),
            }
            Ok(())
        }
    }

    #[test]
    fn withdrawing_staging_preserves_owned_and_foreign_registrations() {
        let mut kernel = Kernel::default();
        kernel.paths.insert("/foreign".into());
        let mut state = Registrations::default();
        state.queue("/mnt/staging");
        state.queue("/system/etc/hosts");
        state.commit(&mut |cmd| kernel.apply(cmd)).unwrap();
        state
            .withdraw("/mnt/staging", &mut |cmd| kernel.apply(cmd))
            .unwrap();
        assert_eq!(
            kernel.paths,
            BTreeSet::from(["/foreign".into(), "/system/etc/hosts".into()])
        );
        assert_eq!(state.committed(), vec!["/system/etc/hosts"]);
    }

    #[test]
    fn withdrawing_before_commit_never_sends_a_kernel_command() {
        let mut state = Registrations::default();
        let mut kernel = Kernel::default();
        state.queue("/mnt/staging");
        state
            .withdraw("/mnt/staging", &mut |cmd| kernel.apply(cmd))
            .unwrap();
        state.commit(&mut |cmd| kernel.apply(cmd)).unwrap();
        assert_eq!(kernel.calls, 0);
    }

    #[test]
    fn failed_withdrawal_retains_ownership_for_retry() {
        let mut state = Registrations::default();
        let mut kernel = Kernel::default();
        state.queue("/system/etc/hosts");
        state.commit(&mut |cmd| kernel.apply(cmd)).unwrap();
        kernel.fail = Some("/system/etc/hosts".into());
        assert!(
            state
                .withdraw("/system/etc/hosts", &mut |cmd| kernel.apply(cmd))
                .is_err()
        );
        assert_eq!(state.committed(), vec!["/system/etc/hosts"]);
        kernel.fail = None;
        state
            .withdraw("/system/etc/hosts", &mut |cmd| kernel.apply(cmd))
            .unwrap();
        assert!(state.committed().is_empty());
    }

    #[test]
    fn partial_commit_rollback_deletes_only_successful_owned_registrations() {
        let mut state = Registrations::default();
        let mut kernel = Kernel::default();
        kernel.paths.insert("/b".into());
        state.queue("/a");
        state.queue("/b");
        assert!(state.commit(&mut |cmd| kernel.apply(cmd)).is_err());
        assert_eq!(state.committed(), vec!["/a"]);
        state.rollback(&mut |cmd| kernel.apply(cmd)).unwrap();
        assert_eq!(kernel.paths, BTreeSet::from(["/b".into()]));
    }

    #[test]
    fn saved_registrations_can_be_released_and_registered_by_a_new_boot_process() {
        let mut kernel = Kernel::default();
        kernel.paths.insert("/foreign".into());
        for _ in 0..2 {
            let mut process = Registrations::default();
            process.queue("/system/etc/hosts");
            process.commit(&mut |cmd| kernel.apply(cmd)).unwrap();
            let owned = process.committed();
            release(&owned, &mut |cmd| kernel.apply(cmd)).unwrap();
            release(&owned, &mut |cmd| kernel.apply(cmd)).unwrap();
        }
        assert_eq!(kernel.paths, BTreeSet::from(["/foreign".into()]));
    }
}
