use clap::{Parser, Subcommand};

use std::env;

static DEFAULT_CIGN_CONFIG_PATH: &'static str = "~/.cign.toml";

#[derive(Parser)]
#[command(author, version, about)]
pub struct Cli {
    #[arg(short, long = "config", help = "Path to cign configuration. Created if not found", default_value = DEFAULT_CIGN_CONFIG_PATH)]
    pub config_path: String,
    #[arg(short, long, help = "Print more info to stdout")]
    pub verbose: bool,
    #[arg(short, long, help = "Fail on errors instead of skipping when possible")]
    pub no_skip: bool,
    #[command(subcommand)]
    pub action: Option<Action>,
}

#[derive(Subcommand)]
pub enum Action {
    /// Add the specified git repo to the config
    Add {
        #[arg(help = "Directory to add", default_value = ".")]
        dir: String,
    },
    /// Remove the specified git repo from the config
    Del {
        #[arg(help = "Directory to remove", default_value = ".")]
        dir: String,
    },
    /// Visit all failing directories one-by-one to fix them
    Fix {
        #[arg(help = "Command to use in each directory", default_value_t = default_fix_cmd())]
        cmd: String,
    },
    /// Show all known repos
    List,
    /// Use a configurable refresh command to e.g. see if all repos are up-to-date with upstream
    Refresh,
}

pub fn default_fix_cmd() -> String {
    env::var("SHELL").unwrap_or("sh".to_string())
}
