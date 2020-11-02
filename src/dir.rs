//! Directory tools

use failure::{format_err, Error};

use crate::{custom::CustomEntry, git::check_repo_in_dir};

/// Returns expanded directories from `dirs_iter`, with `no_skip`
/// deciding if a failed expansion should filter the dir out or fail
/// the whole operation.
pub fn get_expanded_dirs<'a>(
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

        ret.push(expanded_dir);
    }

    Ok(ret)
}

/// Like `get_expanded_dirs` but with additional git checking,
/// yielding those that fail. Skiping also applies to unexpected
/// errors when checking.
pub fn get_failing_expanded_dirs<'a>(
    dirs_iter: impl Iterator<Item = &'a String>,
    no_skip: bool,
) -> Result<Vec<String>, Error> {
    let mut ret = Vec::new();
    let expanded_dirs = get_expanded_dirs(dirs_iter, no_skip)?;
    for expanded_dir in expanded_dirs {
        match check_repo_in_dir(&expanded_dir) {
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
                        expanded_dir, e
                    );
                }
            }
        }
    }
    Ok(ret)
}

pub fn get_failing_custom_entries<'a>(
    entries_iter: impl Iterator<Item = &'a CustomEntry>,
    no_skip: bool,
) -> Result<Vec<CustomEntry>, Error> {
    let mut ret = Vec::<CustomEntry>::new();
    for entry in entries_iter {
        match entry.check() {
            Ok(res) => {
                if !res {
                    ret.push(entry.clone());
                }
            }
            Err(e) => {
                if no_skip {
                    return Err(e);
                } else {
                    warn!(
                        "{}: Skipping because checking failed unexpectedly: {}",
                        entry.name, e
                    );
                }
            }
        }
    }

    Ok(ret)
}
