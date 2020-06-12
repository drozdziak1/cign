//! Command handlers for CLI subcommands

use clap::ArgMatches;
use dialoguer::Confirm;
use failure::{bail, format_err, Error};

use std::{env, ffi, fs, path::Path};

use crate::{config::Config, get_failing_expanded_dirs, git::check_dir, save_cfg};

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

    save_cfg(main_matches, cfg)?;

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

pub fn handle_list(cfg: &Config) {
    println!("{}", cfg.git.iter().cloned().collect::<Vec<_>>().join("\n"));
}