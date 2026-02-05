pub mod fs;
pub mod log;
pub mod process;
pub mod validation;

use std::path::{Path, PathBuf};

pub use self::{fs::*, log::*, process::*, validation::*};

pub fn get_mnt() -> PathBuf {
    let mut name = String::new();

    for _ in 0..10 {
        name.push(fastrand::alphanumeric());
    }

    let path = Path::new("/mnt").join(name);
    path
}
