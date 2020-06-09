#[macro_use]
extern crate log;

#[macro_use]
extern crate serde_derive;

mod config;

use clap::{App, Arg, ArgMatches, SubCommand};
use failure::format_err;
use git2::Repository;
use log::LevelFilter;

use std::{env, fs::File, io::Read};

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

    let cfg = load_cfg(&main_matches)?;

    match main_matches.subcommand() {
        ("add", Some(matches)) => {}
        ("", None) => visit_all_repos(&main_matches, &cfg)?,
        _ => unreachable!(),
    }

    Ok(())
}

fn visit_all_repos(matches: &ArgMatches, cfg: &Config) -> Result<(), ErrBox> {
    for dir in &cfg.git {
	let dir: &str = &shellexpand::full(dir)?;

        debug!("Visiting {}", dir);
        let repo = match Repository::discover(dir) {
            Ok(r) => r,
            Err(e) => {
                warn!("{}: Could not open repo: {}", dir, e);
                continue;
            }
        };
    }

    Ok(())
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
