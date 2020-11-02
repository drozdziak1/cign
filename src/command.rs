//! Command handlers for CLI subcommands

use chrono::Local;
use clap::ArgMatches;
use dialoguer::{Confirm, Input, Select};
use failure::{bail, format_err, Error};
use git2::Repository;

use std::{collections::BTreeMap, env, ffi, fs, path::Path};

use crate::{
    config::Config,
    custom::CustomEntry,
    dir::{get_expanded_dirs, get_failing_custom_entries, get_failing_expanded_dirs},
    git::check_repo_in_dir,
    save_cfg_from_matches,
};

pub fn handle_add(
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
    if p.is_dir() && Repository::discover(&p).is_ok() {
        cfg.git.insert(
            p.to_str()
                .ok_or_else(|| format_err!("INTERNAL: Could not convert path back to string"))?
                .to_owned(),
        );
        save_cfg_from_matches(main_matches, cfg)?;
        println!("Adding {}", p.to_str().unwrap());
    } else {
        return Err(format_err!("{} is not a git repo dir", dir).into());
    }
    Ok(())
}

pub fn handle_add_custom(
    main_matches: &ArgMatches,
    matches: &ArgMatches,
    cfg: &mut Config,
) -> Result<(), Error> {
    let dir: &str = &shellexpand::full(
        matches
            .value_of("DIR")
            .ok_or_else(|| format_err!("INTERNAL: Could not get custom dir to add"))?,
    )?;

    let p = fs::canonicalize(Path::new(dir))?;
    if p.is_dir() {
        let path = p
            .to_str()
            .ok_or_else(|| format_err!("INTERNAL: Could not convert custom path back to string"))?
            .to_owned();

        let default_name = format!("{}", Local::now().format("%Y-%m-%d %H:%M:%S"));

        let name = {
            let mut n;
            loop {
                n = Input::<String>::new()
                    .with_prompt("Name")
                    .default(default_name.clone())
                    .interact()?;

                if cfg.custom.iter().all(|entry| entry.name != n) {
                    break;
                }
            }
            n
        };

        let check_cmd = Input::<String>::new()
            .with_prompt("Custom check command")
            .default("true".to_owned())
            .interact()?;

        let refresh_cmd = Input::<String>::new()
            .with_prompt("Custom refresh command")
            .default("true".to_owned())
            .interact()?;

        println!("Adding custom dir {} with name \"{}\"", path, name);
        cfg.custom.push(CustomEntry {
            name,
            path,
            check_cmd,
            refresh_cmd,
        });
        save_cfg_from_matches(main_matches, cfg)?;
    } else {
        return Err(format_err!("{} is not a git repo dir", dir).into());
    }
    Ok(())
}

pub fn handle_del(
    main_matches: &ArgMatches,
    matches: &ArgMatches,
    cfg: &mut Config,
) -> Result<(), Error> {
    let dir = matches
        .value_of("DIR")
        .ok_or_else(|| format_err!("INTERNAL: Could not get dir to delete"))?;

    if !cfg.git.remove(dir) {
        bail!("No directory named {} in config", dir);
    };

    if Confirm::new()
        .with_prompt(format!("Remove {}?", dir))
        .interact()?
    {
        println!("Removing {}", dir);
        save_cfg_from_matches(main_matches, cfg)?;
    } else {
        bail!("Deletion not confirmed, bailing out.");
    }

    Ok(())
}

pub fn handle_del_custom(
    main_matches: &ArgMatches,
    matches: &ArgMatches,
    cfg: &mut Config,
) -> Result<(), Error> {
    if cfg.custom.is_empty() {
        bail!("No custom entries in config");
    }

    let name2entry: BTreeMap<_, _> = cfg
        .custom
        .iter()
        .map(|entry| (entry.name.clone(), entry.clone()))
        .collect();
    let name = match matches.value_of("NAME") {
        Some(value) => {
            // Check if we have the specified entry
            if !name2entry.contains_key(value) {
                bail!("No custom entry named {} in config", value);
            }
            value.to_owned()
        }
        None => {
            let names: Vec<_> = name2entry.keys().collect();
            // Ask the user to choose an entry
            let idx = Select::new()
                .items(&names)
                .default(0)
                .with_prompt("Choose custom entry to delete")
                .interact()?;

            names[idx].to_owned()
        }
    };

    cfg.custom = cfg
        .custom
        .drain(0..)
        .filter(|entry| entry.name != name)
        .collect();

    if Confirm::new()
        .with_prompt(format!("Remove {} ({})?", name, name2entry[&name].path))
        .interact()?
    {
        println!("Removing {} ({})", name, name2entry[&name].path);
        save_cfg_from_matches(main_matches, cfg)?;
    } else {
        bail!("Deletion not confirmed, bailing out.");
    }

    Ok(())
}

pub fn handle_fix(
    main_matches: &ArgMatches,
    matches: &ArgMatches,
    cfg: &mut Config,
) -> Result<(), Error> {
    let cmd = matches
        .value_of("CMD")
        .ok_or_else(|| format_err!("INTERNAL: could not get CMD"))?;

    let failing_expanded_dirs =
        get_failing_expanded_dirs(cfg.git.iter(), main_matches.is_present("no-skip"))?;

    let failing_count = failing_expanded_dirs.len();

    if failing_count > 0 {
        println!("Fixing {} git dirs", failing_count);
    }

    // Save current dir
    let cwd = env::current_dir()?;

    // Go through each failing dir executing CMD
    for (idx, dir) in failing_expanded_dirs.iter().enumerate() {
        loop {
            // Change to the directory
            env::set_current_dir(Path::new(&dir))?;

            println!(
                "{}/{}: Fixing git dir {}",
                idx + 1,
                failing_expanded_dirs.len(),
                dir
            );

            // Run the command
            let command_result: libc::c_int;
            unsafe {
                command_result = libc::WEXITSTATUS(libc::system(ffi::CString::new(cmd)?.as_ptr()));
            }

            if command_result != libc::EXIT_SUCCESS {
                warn!("{}: command exited with code {}", dir, command_result);
            }

            // Re-run the check
            let check_res = check_repo_in_dir(dir)?;

            // Loop back if it's still failing and they want to retry
            if !check_res.is_all_good() {
                if Confirm::new()
                    .with_prompt(format!(
                        "{}: still failing ({}). Retry?",
                        dir,
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
            dir
        );
    }

    // Restore previous working directory
    env::set_current_dir(cwd)?;

    Ok(())
}

pub fn handle_fix_custom(
    main_matches: &ArgMatches,
    matches: &ArgMatches,
    cfg: &mut Config,
) -> Result<(), Error> {
    let cmd = matches
        .value_of("CMD")
        .ok_or_else(|| format_err!("INTERNAL: could not get CMD"))?;

    let failing_custom_entries =
        get_failing_custom_entries(cfg.custom.iter(), main_matches.is_present("no-skip"))?;

    let failing_count = failing_custom_entries.len();

    if failing_count > 0 {
        println!("Fixing {} custom dirs", failing_count);
    }

    // Save current dir
    let cwd = env::current_dir()?;

    for (idx, entry) in failing_custom_entries.iter().enumerate() {
        let dir: &str = &shellexpand::full(&entry.path)?;

        loop {
            // Change to the directory
            env::set_current_dir(Path::new(&dir))?;

            println!(
                "{}/{}: Fixing custom dir {:?} ({}, check_cmd: {:?})",
                idx + 1,
                failing_count,
                entry.name,
                dir,
                entry.check_cmd,
            );
            // Run the command
            let command_result: libc::c_int;
            unsafe {
                command_result = libc::WEXITSTATUS(libc::system(ffi::CString::new(cmd)?.as_ptr()));
            }

            if command_result != libc::EXIT_SUCCESS {
                warn!("{}: command exited with code {}", dir, command_result);
            }

            // Loop back if it's still failing and they want to retry
            if !entry.check()? {
                if Confirm::new()
                    .with_prompt(format!("{}: still failing. Retry?", entry.name,))
                    .interact()?
                {
                    continue;
                }
            }

            break;
        }
        println!(
            "{}/{}: Leaving {:?} ({})",
            idx + 1,
            failing_count,
            entry.name,
            dir
        );
    }

    // Restore previous working directory
    env::set_current_dir(cwd)?;

    Ok(())
}

pub fn handle_list(cfg: &Config) {
    println!("{}", cfg.git.iter().cloned().collect::<Vec<_>>().join("\n"));
    println!(
        "{}",
        cfg.custom
            .iter()
            .cloned()
            .map(|entry| format!("{} ({})", entry.name, entry.path))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

pub fn handle_refresh(main_matches: &ArgMatches, cfg: &Config) -> Result<(), Error> {
    let no_skip = main_matches.is_present("no-skip");
    let expanded_dirs = get_expanded_dirs(cfg.git.iter(), no_skip)?;

    for (idx, dir) in expanded_dirs.iter().enumerate() {
        println!(
            "[{}/{}] Refreshing git dir {}:",
            idx + 1,
            expanded_dirs.len(),
            dir
        );
        let repo = match Repository::discover(&dir) {
            Ok(r) => r,
            Err(e) => {
                if no_skip {
                    bail!("{}: Could not open repo: {}", dir, e);
                } else {
                    warn!("{}: Skipping, opening the repo failed: {}", dir, e);
                    continue;
                }
            }
        };

        // Shell out for the configurable refresh command. Users might
        // prefer something lighter than a full `git remote update`.
        let command_result: libc::c_int;
        unsafe {
            command_result = libc::WEXITSTATUS(libc::system(
                ffi::CString::new(format!(
                    "GIT_DIR={} {}",
                    repo.path().display(),
                    cfg.refresh_cmd
                ))?
                .as_ptr(),
            ));
        }

        if command_result != libc::EXIT_SUCCESS {
            warn!(
                "{}: refresh command exited with code {}",
                dir, command_result
            );
        }
    }

    let custom_count = cfg.custom.len();
    cfg.custom
        .iter()
        .enumerate()
        .map(|(idx, entry)| {
            println!(
                "[{}/{}] Refreshing custom dir {:?} ({}, refresh_cmd: {:?}):",
                idx + 1,
                custom_count,
                entry.name,
                entry.path,
                entry.refresh_cmd,
            );
            entry.refresh()
        })
        .collect::<Result<_, Error>>()?;

    Ok(())
}
