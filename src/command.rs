//! Command handlers for CLI subcommands

use dialoguer::Confirm;
use git2::Repository;
use log::warn;

use std::{env, ffi, path::Path};

use crate::{
    config::Config,
    dir::{get_expanded_dirs, get_failing_expanded_dirs},
    git::check_repo,
    ErrBox,
};

pub fn handle_add(cfg: &mut Config, new_dir: &Path) -> Result<(), ErrBox> {
    if new_dir.is_dir() {
        cfg.git.insert(new_dir.display().to_string());
        println!("Adding {}", new_dir.to_str().unwrap());
        Ok(())
    } else {
        Err(format!("{} is not a directory", new_dir.display()).into())
    }
}

pub fn handle_del(cfg: &mut Config, dir_to_delete: &Path) -> Result<(), ErrBox> {
    if !cfg
        .git
        .contains(dir_to_delete.display().to_string().as_str())
    {
        return Err(format!("{} not in config", dir_to_delete.display()).into());
    };

    if Confirm::new()
        .with_prompt(format!("Remove {}?", dir_to_delete.display()))
        .interact()?
    {
        cfg.git.remove(dir_to_delete.display().to_string().as_str());
        Ok(())
    } else {
        return Err("Deletion not confirmed, bailing out.".into());
    }
}

pub fn handle_fix(cfg: &Config, fix_cmd: &str, no_skip: bool) -> Result<(), ErrBox> {
    let failing_expanded_dirs = get_failing_expanded_dirs(cfg.git.iter(), no_skip)?;

    // Save current dir
    let cwd = env::current_dir()?;

    // Go through each failing dir executing CMD
    for (idx, dir) in failing_expanded_dirs.iter().enumerate() {
        let repo = Repository::discover(dir)?;
        loop {
            // Change to the directory
            env::set_current_dir(Path::new(&dir))?;

            println!(
                "{}/{}: Fixing {}",
                idx + 1,
                failing_expanded_dirs.len(),
                dir
            );

            // Run the command
            let command_result: libc::c_int;
            unsafe {
                command_result =
                    libc::WEXITSTATUS(libc::system(ffi::CString::new(fix_cmd)?.as_ptr()));
            }

            if command_result != libc::EXIT_SUCCESS {
                warn!("{}: command exited with code {}", dir, command_result);
            }

            // Re-run the check
            let check_res = check_repo(&repo)?;

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

pub fn handle_list(cfg: &Config) {
    println!("{}", cfg.git.iter().cloned().collect::<Vec<_>>().join("\n"));
}

pub fn handle_refresh(cfg: &Config, no_skip: bool) -> Result<(), ErrBox> {
    let expanded_dirs = get_expanded_dirs(cfg.git.iter(), no_skip)?;

    for (idx, dir) in expanded_dirs.iter().enumerate() {
        println!("[{}/{}] Refreshing {}:", idx + 1, expanded_dirs.len(), dir);
        let repo = match Repository::discover(&dir) {
            Ok(r) => r,
            Err(e) => {
                if no_skip {
                    return Err(format!("{}: Could not open repo: {}", dir, e).into());
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

    Ok(())
}
