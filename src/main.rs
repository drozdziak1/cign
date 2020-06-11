#[macro_use]
extern crate log;

#[macro_use]
extern crate serde_derive;

mod config;

use clap::{App, Arg, ArgMatches, SubCommand};
use failure::format_err;
use git2::{Repository, RepositoryState, StatusOptions};
use log::LevelFilter;

use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
    process,
};

use config::Config;

pub type ErrBox = Box<dyn std::error::Error>;

static DEFAULT_CIGN_CONFIG_PATH: &'static str = "~/.cign.toml";

fn main() -> Result<(), ErrBox> {
    init_log();

    let main_matches = App::new(env!("CARGO_PKG_NAME"))
        .about("cign = Can I Go Now? cign is a friendly reminder program for your unpushed code.")
        .version(env!("CARGO_PKG_VERSION"))
        .arg(
            Arg::with_name("config")
                .help("Path to cign configuration")
                .short("c")
                .long("config")
                .value_name("CIGN_CONFIG")
                .default_value(DEFAULT_CIGN_CONFIG_PATH),
        )
        .subcommand(
            SubCommand::with_name("add")
                .about("Add a new directory to the watchlist")
                .arg(
                    Arg::with_name("DIR")
                        .default_value(".")
                        .help("The directory to add"),
                ),
        )
        .get_matches();

    let mut cfg = load_cfg(&main_matches)?;

    match main_matches.subcommand() {
        ("add", Some(matches)) => {
            handle_add(&main_matches, matches, &mut cfg)?;
        }
        ("", None) => {
            if visit_all_repos(&main_matches, &cfg)? {
                println!("OK.");
            } else {
                process::exit(1);
            }
        }
        _ => unreachable!(),
    }

    Ok(())
}

fn handle_add(
    main_matches: &ArgMatches,
    matches: &ArgMatches,
    cfg: &mut Config,
) -> Result<(), ErrBox> {
    let dir: &str = &shellexpand::full(
        matches
            .value_of("DIR")
            .ok_or_else(|| format_err!("INTERNAL: Could not get dir to add"))?,
    )?;

    let p = fs::canonicalize(Path::new(dir))?;
    if p.is_dir() {
        cfg.git.insert(
            p.to_str()
                .ok_or_else(|| format_err!("INTERNAL: Could not convert path back to string"))?
                .to_owned(),
        );
        save_cfg(main_matches, cfg)?;
        println!("Adding {}", p.to_str().unwrap());
    } else {
        return Err(format_err!("{} is not a directory", dir).into());
    }
    Ok(())
}

/// Returns false if any of the configured repos is dirty
fn visit_all_repos(_main_matches: &ArgMatches, cfg: &Config) -> Result<bool, ErrBox> {
    let mut clean = true;

    for dir in &cfg.git {
        let expanded_dir: &str = &shellexpand::full(dir)?;

        debug!("Visiting {}", expanded_dir);
        let repo = match Repository::discover(expanded_dir) {
            Ok(r) => r,
            Err(e) => {
                warn!("{}: Could not open repo: {}", dir, e);
                continue;
            }
        };

        let state = repo.state();
        let change_count = repo
            .statuses(Some(StatusOptions::new().include_ignored(false)))?
            .iter()
            .inspect(|entry| trace!("{}: {:?}: status {:?}", dir, entry.path(), entry.status()))
            .count();

        let is_unclean = state != RepositoryState::Clean;
        let has_changes = change_count > 0;

        if has_changes || is_unclean {
            clean = false;
            print!("{}: ", dir);

            if has_changes {
                print!(
                    "{} uncommitted change{}{}",
                    change_count,
                    if change_count == 1 { "" } else { "s" },
                    if is_unclean { ", " } else { "" }
                );
            }

            if is_unclean {
                print!("non-clean state {:?}", state);
            }

            println!("");
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

fn load_cfg(matches: &ArgMatches) -> Result<Config, ErrBox> {
    let fname: &str = &shellexpand::full(
        matches
            .value_of("config")
            .ok_or_else(|| format_err!("INTERNAL: could not obtain config path"))?,
    )?;

    let mut buf = String::new();
    let mut file = File::open(fname)?;

    file.read_to_string(&mut buf)?;

    let cfg = toml::from_str(&buf)?;

    debug!("Config:\n{:#?}", cfg);

    Ok(cfg)
}

fn save_cfg(matches: &ArgMatches, cfg: &Config) -> Result<(), ErrBox> {
    let fname: &str = &shellexpand::full(
        matches
            .value_of("config")
            .ok_or_else(|| format_err!("INTERNAL: could not obtain config path"))?,
    )?;

    let mut f = OpenOptions::new().write(true).open(fname)?;

    f.write_all(toml::to_vec(cfg)?.as_slice())?;
    Ok(())
}
