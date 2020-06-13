//! TOML config schema

use std::collections::BTreeSet;

static DEFAULT_REFRESH_CMD: &'static str = "git remote update";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub git: BTreeSet<String>,
    #[serde(default = "default_refresh_cmd")]
    pub refresh_cmd: String,
}

#[inline]
fn default_refresh_cmd() -> String {
    DEFAULT_REFRESH_CMD.to_owned()
}

impl Default for Config {
    fn default() -> Self {
	Config {
	    git: BTreeSet::new(),
	    refresh_cmd: default_refresh_cmd(),
	}
    }
}
