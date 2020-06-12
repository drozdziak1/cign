#[macro_use]
extern crate log;

#[macro_use]
extern crate serde_derive;

mod config;
mod git;

use clap::{App, Arg, ArgMatches, SubCommand};
use dialoguer::Confirm;
use failure::{format_err, Error};
use log::LevelFilter;

use std::{
    env, ffi,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
    process,
};

use config::Config;
use git::check_dir;

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
                .about("Add a new directory to the watchlist")
                .arg(
                    Arg::with_name("DIR")
                        .default_value(".")
                        .help("The directory to add"),
                ),
        )
        .subcommand(
            SubCommand::with_name("fix")
                .about("Run CMD in each dir to let the user get it back to clean state")
                .arg(
                    Arg::with_name("CMD")
                        .default_value(&current_shell)
                        .help("The program to run in each failing directory"),
                ),
        )
        .get_matches();

    let mut cfg = load_cfg(&main_matches)?;

    match main_matches.subcommand() {
        ("add", Some(matches)) => {
            handle_add(&main_matches, matches, &mut cfg)?;
        }
        ("fix", Some(matches)) => {
            handle_fix(&main_matches, matches, &mut cfg)?;
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

fn handle_add(
    main_matches: &ArgMatches,
    matches: &ArgMatches,
    cfg: &mut Config,
) -> Result<(), Error> {
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

/// Traverses `dirs_iter` and returns an expanded dir vector with all that failed the dir checks.
fn get_failing_expanded_dirs<'a>(
    dirs_iter: impl Iterator<Item = &'a String>,
    no_skip: bool,
) -> Result<Vec<String>, Error> {
    let mut ret = Vec::new();
    for dir in dirs_iter {
        let expanded_dir: String = match &shellexpand::full(&dir) {
            Ok(d) => d.as_ref().to_owned(),
            Err(e) => {
                if no_skip {
                    return Err(format_err!("{}: Could not expand dir: {}", dir, e));
                } else {
                    warn!("{}: Skipping because expanding the path failed: {}", dir, e);
                    continue;
                }
            }
        };

        match check_dir(&expanded_dir) {
            Ok(chk_res) => {
                if !chk_res.is_all_good() {
                    ret.push(expanded_dir);
                }
            }
            Err(e) => {
                if no_skip {
                    return Err(e);
                } else {
                    warn!(
                        "{}: Skipping because checking failed unexpectedly: {}",
                        dir, e
                    );
                    continue;
                }
            }
        }
    }
    Ok(ret)
}

fn handle_fix(
    main_matches: &ArgMatches,
    matches: &ArgMatches,
    cfg: &mut Config,
) -> Result<(), Error> {
    let cmd = matches
        .value_of("CMD")
        .ok_or_else(|| format_err!("INTERNAL: could not get CMD"))?;

    let failing_expanded_dirs =
        get_failing_expanded_dirs(cfg.git.iter(), main_matches.is_present("no-skip"))?;

    // Save current dir
    let cwd = env::current_dir()?;

    // Go through each failing dir executing CMD
    for (idx, expanded_dir) in failing_expanded_dirs.iter().enumerate() {
        loop {
            // Change to the directory
            env::set_current_dir(Path::new(&expanded_dir))?;

            println!(
                "{}/{}: Fixing {}",
                idx + 1,
                failing_expanded_dirs.len(),
                expanded_dir
            );

            // Run the command
            let command_result: libc::c_int;
            unsafe {
                command_result = libc::WEXITSTATUS(libc::system(ffi::CString::new(cmd)?.as_ptr()));
            }

            if command_result != libc::EXIT_SUCCESS {
                warn!(
                    "{}: command exited with code {}",
                    expanded_dir, command_result
                );
            }

            // Re-run the check
            let check_res = check_dir(expanded_dir)?;

            // Loop back if it's still failing and they want to retry
            if !check_res.is_all_good() {
                if Confirm::new()
                    .with_prompt(format!(
                        "{}: still failing ({}). Retry?",
                        expanded_dir,
                        check_res.describe().join(", "),
                    ))
                    .interact()?
                {
                    continue;
                }
            }

            break;
        }

        println!(
            "{}/{}: Leaving {}",
            idx + 1,
            failing_expanded_dirs.len(),
            expanded_dir
        );
    }

    // Restore previous working directory
    env::set_current_dir(cwd)?;

    Ok(())
}

/// Returns false if any of the configured repos is dirty
fn visit_all_repos(main_matches: &ArgMatches, cfg: &Config) -> Result<bool, Error> {
    let mut clean = true;

    for dir in &cfg.git {
        let expanded_dir: &str = &shellexpand::full(dir)?;

        debug!("Visiting {}", expanded_dir);

        let check_result = check_dir(expanded_dir)?;

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

fn load_cfg(matches: &ArgMatches) -> Result<Config, Error> {
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

fn save_cfg(matches: &ArgMatches, cfg: &Config) -> Result<(), Error> {
    let fname: &str = &shellexpand::full(
        matches
            .value_of("config")
            .ok_or_else(|| format_err!("INTERNAL: could not obtain config path"))?,
    )?;

    let mut f = OpenOptions::new().write(true).open(fname)?;

    f.write_all(toml::to_vec(cfg)?.as_slice())?;
    Ok(())
}
