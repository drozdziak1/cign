//! TOML config schema

use std::collections::BTreeSet;

use crate::custom::CustomEntry;

static DEFAULT_REFRESH_CMD: &'static str = "git remote update";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_refresh_cmd")]
    pub refresh_cmd: String,
    pub enable_chad: Option<String>,
    #[serde(default)]
    pub git: BTreeSet<String>,
    #[serde(default)]
    pub custom: Vec<CustomEntry>,
}

#[inline]
fn default_refresh_cmd() -> String {
    DEFAULT_REFRESH_CMD.to_owned()
}

impl Default for Config {
    fn default() -> Self {
        Config {
            git: Default::default(),
            custom: Default::default(),
            refresh_cmd: default_refresh_cmd(),
            enable_chad: None,
        }
    }
}
