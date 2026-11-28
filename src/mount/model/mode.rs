use serde::{Deserialize, Serialize};

/// Mount backend selectable per path or per module.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Overlay,
    Magic,
    Vfs,
    Ignore,
}

impl Mode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Overlay => "overlay",
            Self::Magic => "magic",
            Self::Vfs => "vfs",
            Self::Ignore => "ignore",
        }
    }
}
