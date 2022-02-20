mod cli;
mod command;
mod config;
mod dir;
mod git;

use clap::Parser;
use cli::{Action, Cli};
use failure::Error;
use log::{debug, warn, LevelFilter};

use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process,
};

use config::Config;
use git::check_repo_in_dir;

pub type ErrBox = Box<dyn std::error::Error>;

fn main() -> Result<(), ErrBox> {
    init_log();

    let cli = Cli::parse();

    // Expand the config path
    let config_path = Path::new(&*shellexpand::full(&cli.config_path)?).to_owned();

    let mut cfg: Config = if config_path.exists() {
        load_cfg(&config_path)?
    } else {
        println!(
            "Config not found, initializing in {}...",
            config_path.display()
        );
        let cfg = Default::default();
        save_cfg(&cfg, &config_path)?;
        cfg
    };

    match &cli.action {
        Some(Action::Add { dir }) => {
            let dir = fs::canonicalize(Path::new(&*shellexpand::full(&dir)?).to_owned())?;
            command::handle_add(&mut cfg, &dir)?;
        }
        Some(Action::Del { dir }) => {
            let dir = fs::canonicalize(Path::new(&*shellexpand::full(&dir)?).to_owned())?;
            command::handle_del(&mut cfg, &dir)?;
        }
        Some(Action::Fix { cmd }) => {
            command::handle_fix(&cfg, cmd, cli.no_skip)?;
        }
        Some(Action::List) => {
            command::handle_list(&cfg);
        }
        Some(Action::Refresh) => {
            command::handle_refresh(&cfg, cli.no_skip)?;
        }
        None => {
            if check_all_repos(&cfg, cli.no_skip, cli.verbose)? {
                if cfg.enable_chad == Some("Yes.".to_owned()) {
                    eprintln!("{}", include_str!("../assets/chad.txt"));
                } else {
                    eprintln!("OK");
                }
            } else {
                process::exit(1);
            }
        }
    }

    save_cfg(&cfg, &config_path)?;

    Ok(())
}

/// Returns false if any of the configured repos is dirty
fn check_all_repos(cfg: &Config, no_skip: bool, verbose: bool) -> Result<bool, ErrBox> {
    let mut clean = true;

    for dir in &cfg.git {
        let expanded_dir: PathBuf = Path::new(&*shellexpand::full(dir)?).to_owned();

        debug!("Visiting {}", expanded_dir.display());

        match check_repo_in_dir(expanded_dir.as_path()) {
            Ok(check_result) => {
                if !check_result.is_all_good() || verbose {
                    println!("{}: {}", dir, check_result.describe().join(" | "));
                }

                if !check_result.is_all_good() && no_skip {
                    println!("No-skip mode is on, exitting...");
                    return Ok(false);
                }

                clean = clean && check_result.is_all_good();
            }
            Err(e) => {
                warn!("Checking {} failed unexpectedly: {}", dir, e);
                clean = false;
            }
        }
    }

    Ok(clean)
}

/// Init logging at info level
fn init_log() {
    match env::var("RUST_LOG") {
        Ok(_value) => env_logger::init(),
        Err(_e) => env_logger::Builder::new()
            .filter_level(LevelFilter::Info)
            .init(),
    }
}

/// Load config from the specified path.
pub fn load_cfg(cfg_path: &Path) -> Result<Config, Error> {
    let mut buf = String::new();
    let mut file = File::open(cfg_path)?;

    file.read_to_string(&mut buf)?;

    let cfg = toml::from_str(&buf)?;

    debug!("Config:\n{:#?}", cfg);

    Ok(cfg)
}

/// Save `cfg` to the path specified in `matches`.
pub fn save_cfg(cfg: &Config, cfg_path: &Path) -> Result<(), Error> {
    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(cfg_path)?;

    f.write_all(toml::to_vec(cfg)?.as_slice())?;
    Ok(())
}
