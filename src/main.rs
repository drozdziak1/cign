mod cli;
mod command;
mod config;
mod dir;
mod git;

use clap::Parser;
use cli_table::{format::Separator, Cell, Table};
use failure::Error;
use log::{debug, warn, LevelFilter};

use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process,
};

use crate::git::find_git_repos_recursive;
use cli::{Action, Cli};
use config::Config;
use git::{check_repo, GitCheckResult};

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
            let mut results = check_all_repos(&cfg)?;

            let (mut good_repos, bad_repos): (Vec<_>, Vec<_>) =
                results.drain(..).partition(|result| result.1.is_all_good());

            let table_separator = Separator::builder()
                .title(Some(Default::default()))
                .row(None)
                .column(Some(Default::default()))
                .build();

            if bad_repos.is_empty() {
                if cfg.enable_chad == Some("Yes.".to_owned()) {
                    eprintln!("{}", include_str!("../assets/chad.txt"));
                } else {
                    eprintln!("OK");
                }
            } else {
                let mut table_rows = vec![];
                for (bad_path, bad_result) in bad_repos.clone() {
                    let row = vec![
                        format!("{}", bad_path.display()).cell(),
                        bad_result.describe().join(" - ").cell(),
                    ];
                    table_rows.push(row);
                }

                let table = table_rows
                    .table()
                    .title(vec!["Path".cell(), "Problem".cell()])
                    .separator(table_separator);

                println!("{}", table.display()?);
            }

            if cli.verbose {
                let table_rows: Vec<_> = good_repos
                    .drain(..)
                    .map(|(p, _r)| vec![format!("{}", p.display()).cell()])
                    .collect();

                let table = table_rows
                    .table()
                    .title(vec!["Clean Repos"])
                    .separator(table_separator);

                println!("{}", table.display()?);
            }

	    if !bad_repos.is_empty() {
                process::exit(1);
	    }
        }
    }

    save_cfg(&cfg, &config_path)?;

    Ok(())
}

/// Returns false if any of the configured repos is dirty
fn check_all_repos(cfg: &Config) -> Result<Vec<(PathBuf, GitCheckResult)>, ErrBox> {
    let all_repos = cfg
        .git
        .iter()
        .map(|d| {
            shellexpand::full(d)
                .map_err(|e| -> ErrBox { e.into() })
                .and_then(|expanded| find_git_repos_recursive(PathBuf::from(expanded.as_ref())))
                .map_err(|e| {
                    warn!("Could not look for git repos in {}: {} ", d, e.to_string());
                    e
                })
        })
        .filter_map(|res| res.ok())
        .flatten();

    let mut results = Vec::new();

    for repo in all_repos {
        let repo_dir = repo.workdir().unwrap_or(repo.path());
        debug!("Visiting {}", repo_dir.display());
        match check_repo(&repo) {
            Ok(check_result) => {
                results.push((PathBuf::from(repo_dir), check_result));
            }
            Err(e) => {
                warn!("Checking {} failed unexpectedly: {}", repo_dir.display(), e);
            }
        }
    }

    Ok(results)
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
