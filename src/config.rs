//! TOML config schema

use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub git: BTreeSet<String>,
}
