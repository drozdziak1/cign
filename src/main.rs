#[macro_use]
extern crate log;

#[macro_use]
extern crate serde_derive;

mod command;
mod config;
mod custom;
mod dir;
mod git;

use clap::{App, Arg, ArgMatches, SubCommand};
use failure::{format_err, Error};
use log::LevelFilter;

use std::{
    env,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
    process,
};

use config::Config;
use git::check_repo_in_dir;

pub type ErrBox = Box<dyn std::error::Error>;

static DEFAULT_CIGN_CONFIG_PATH: &'static str = "~/.cign.toml";

fn main() -> Result<(), ErrBox> {
    init_log();

    let current_shell = env::var("SHELL")?;

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
        .arg(
            Arg::with_name("verbose")
                .help("Print more info to stdout")
                .short("v")
                .long("verbose")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("no-skip")
                .help("Fail on errors instead of skipping when possible")
                .short("s")
                .long("no-skip")
                .takes_value(false),
        )
        .subcommand(
            SubCommand::with_name("add")
                .alias("a")
                .about("Add a new directory to the config")
                .arg(
                    Arg::with_name("DIR")
                        .default_value(".")
                        .help("The directory to add"),
                ),
        )
        .subcommand(
            SubCommand::with_name("add-custom")
                .alias("ac")
                .about("Prompt for a new custom directory to the config")
                .arg(
                    Arg::with_name("DIR")
                        .default_value(".")
                        .help("The custom directory to add"),
                ),
        )
        .subcommand(
            SubCommand::with_name("del")
                .alias("d")
                .about("Remove a directory from the config")
                .arg(
                    Arg::with_name("DIR")
                        .default_value(".")
                        .help("The directory to delete"),
                ),
        )
        .subcommand(
            SubCommand::with_name("del-custom")
                .alias("dc")
                .about("Remove a custom directory from the config")
                .arg(Arg::with_name("NAME").help("The custom entry name to delete")),
        )
        .subcommand(
            SubCommand::with_name("fix")
                .alias("f")
                .about("Run CMD in each dir to let the user get it back to clean state")
                .arg(
                    Arg::with_name("CMD")
                        .default_value(&current_shell)
                        .help("The command to run in each failing directory"),
                ),
        )
        .subcommand(
            SubCommand::with_name("init").alias("i").about(
                format!(
                    "Initialize a default config in {}",
                    DEFAULT_CIGN_CONFIG_PATH
                )
                .as_str(),
            ),
        )
        .subcommand(
            SubCommand::with_name("list")
                .alias("l")
                .about("List all configured directories"),
        )
        .subcommand(
            SubCommand::with_name("refresh")
                .alias("r")
                .about("Fetch upstream refs for every repo"),
        )
        .get_matches();

    if let ("init", Some(_)) = main_matches.subcommand() {
        let dir = Path::new(
            main_matches
                .value_of("config")
                .ok_or_else(|| format_err!("INTERNAL: could not get config path"))?,
        );

        if dir.exists() {
            println!("Config exists");
            process::exit(1);
        } else {
            save_cfg_from_matches(&main_matches, &Default::default())?;
            return Ok(());
        }
    }

    let mut cfg = load_cfg_from_matches(&main_matches)?;

    match main_matches.subcommand() {
        ("add", Some(matches)) => {
            command::handle_add(&main_matches, matches, &mut cfg)?;
        }
        ("add-custom", Some(matches)) => {
            command::handle_add_custom(&main_matches, matches, &mut cfg)?;
        }
        ("del", Some(matches)) => {
            command::handle_del(&main_matches, matches, &mut cfg)?;
        }
        ("del-custom", Some(matches)) => {
            command::handle_del_custom(&main_matches, matches, &mut cfg)?;
        }
        ("fix", Some(matches)) => {
            command::handle_fix(&main_matches, matches, &mut cfg)?;
            command::handle_fix_custom(&main_matches, matches, &mut cfg)?;
        }
        ("list", Some(_)) => {
            command::handle_list(&mut cfg);
        }
        ("refresh", Some(_)) => {
            command::handle_refresh(&main_matches, &mut cfg)?;
        }
        ("", None) => {
            if visit_all_repos(&main_matches, &cfg)? {
                if cfg.enable_chad == Some("Yes.".to_owned()) {
                    eprintln!("{}", include_str!("../assets/chad.txt"));
                } else {
                    eprintln!("OK");
                }
            } else {
                process::exit(1);
            }
        }
        _ => unreachable!(),
    }

    Ok(())
}

/// Returns false if any of the configured repos is dirty
fn visit_all_repos(main_matches: &ArgMatches, cfg: &Config) -> Result<bool, Error> {
    let mut clean = true;

    for dir in &cfg.git {
        let expanded_dir: &str = &shellexpand::full(dir)?;

        debug!("Visiting git repo {}", expanded_dir);

        match check_repo_in_dir(expanded_dir) {
            Ok(check_result) => {
                if !check_result.is_all_good() || main_matches.is_present("verbose") {
                    println!("{}: {}", dir, check_result.describe().join(" | "));
                }

                clean = clean && check_result.is_all_good();
            }
            Err(e) => {
                warn!("Checking {} failed unexpectedly: {}", dir, e);
                clean = false;
            }
        }
    }

    for custom_entry in &cfg.custom {
        debug!(
            "Visiting custom dir {} ({})",
            custom_entry.name, custom_entry.path
        );

        match custom_entry.check() {
            Ok(res) => {
                if !res {
                    println!("{}: Dirty", custom_entry.name);
                    clean = false;
                } else if main_matches.is_present("verbose") {
                    println!("{}: OK", custom_entry.path);
                }
            }
            Err(e) => {
                warn!("Checking {} failed unexpectedly: {}", custom_entry.name, e);

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

/// Load config from the path specified in `matches`.
pub fn load_cfg_from_matches(matches: &ArgMatches) -> Result<Config, Error> {
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

/// Save `cfg` to the path specified in `matches`.
pub fn save_cfg_from_matches(matches: &ArgMatches, cfg: &Config) -> Result<(), Error> {
    let fname: &str = &shellexpand::full(
        matches
            .value_of("config")
            .ok_or_else(|| format_err!("INTERNAL: could not obtain config path"))?,
    )?;

    let config_string = toml::to_vec(cfg)?;

    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(fname)?;

    f.write_all(config_string.as_slice())?;
    Ok(())
}
