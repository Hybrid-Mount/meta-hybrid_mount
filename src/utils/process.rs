// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    ffi::CString,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;

pub fn camouflage_process(name: &str) -> Result<()> {
    let c_name = CString::new(name)?;
    unsafe {
        libc::prctl(libc::PR_SET_NAME, c_name.as_ptr() as u64, 0, 0, 0);
    }
    Ok(())
}

pub fn random_kworker_name() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let x = nanos % 16;
    let y = (nanos >> 4) % 10;
    format!("kworker/u{}:{}", x, y)
}
