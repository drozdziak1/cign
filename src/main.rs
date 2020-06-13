#[macro_use]
extern crate log;

#[macro_use]
extern crate serde_derive;

mod command;
mod config;
mod dir;
mod git;

use clap::{App, Arg, ArgMatches, SubCommand};
use failure::{format_err, Error};
use log::LevelFilter;

use std::{
    env,
    fs::{File, OpenOptions},
    io::{Read, Write},
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
            SubCommand::with_name("del")
                .alias("d")
                .about("Removes a directory from the config")
                .arg(
                    Arg::with_name("DIR")
                        .default_value(".")
                        .help("The directory to delete"),
                ),
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

    let mut cfg = load_cfg_from_matches(&main_matches)?;

    match main_matches.subcommand() {
        ("add", Some(matches)) => {
            command::handle_add(&main_matches, matches, &mut cfg)?;
        }
        ("del", Some(matches)) => {
            command::handle_del(&main_matches, matches, &mut cfg)?;
        }
        ("fix", Some(matches)) => {
            command::handle_fix(&main_matches, matches, &mut cfg)?;
        }
        ("list", Some(_)) => {
            command::handle_list(&mut cfg);
        }
        ("refresh", Some(_)) => {
            command::handle_refresh(&mut cfg)?;
        }
        ("", None) => {
            if visit_all_repos(&main_matches, &cfg)? {
                println!("OK");
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

        debug!("Visiting {}", expanded_dir);

        let check_result = check_repo_in_dir(expanded_dir)?;

        if !check_result.is_all_good() || main_matches.is_present("verbose") {
            println!("{}: {}", dir, check_result.describe().join(" | "));
        }

        clean = clean && check_result.is_all_good();
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

    let mut f = OpenOptions::new().write(true).truncate(true).open(fname)?;

    f.write_all(toml::to_vec(cfg)?.as_slice())?;
    Ok(())
}
