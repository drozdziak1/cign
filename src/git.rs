//! Git directory checking helpers

use git2::{Repository, RepositoryState, Status, StatusOptions};
use log::{debug, trace};

use std::{collections::VecDeque, fs, path::Path};

use crate::ErrBox;

/// Encapsulates different things that can fail when checking a repo.
#[derive(Clone, Debug)]
pub struct GitCheckResult {
    pub state: RepositoryState,
    pub statuses: Vec<Status>,
    /// How many commmits ahead of upstream
    pub commits_ahead: usize,
    /// How many commmits behind upstream
    pub commits_behind: usize,
}

impl GitCheckResult {
    /// True if no unfinished business was detected
    pub fn is_all_good(&self) -> bool {
        self.state == RepositoryState::Clean
            && self.statuses.is_empty()
            && self.commits_ahead == 0
            && self.commits_behind == 0
    }

    /// Returns a list of facts about the result.
    pub fn describe(&self) -> Vec<String> {
        if self.is_all_good() {
            return vec!["all good".to_owned()];
        }

        let mut ret = Vec::new();

        if self.state != RepositoryState::Clean {
            ret.push(format!("unclean state {:?}", self.state));
        }

        let change_count = self.statuses.len();
        if change_count > 0 {
            ret.push(format!("{} uncommitted change(s)", change_count,));
        }

        if self.commits_ahead > 0 {
            ret.push(format!("{} commit(s) ahead", self.commits_ahead));
        }

        if self.commits_behind > 0 {
            ret.push(format!("{} commit(s) behind", self.commits_behind));
        }

        ret
    }
}

pub fn check_repo(repo: &Repository) -> Result<GitCheckResult, ErrBox> {
    let state = repo.state();
    let statuses = repo
        .statuses(Some(StatusOptions::new().include_ignored(false)))?
        .iter()
        .map(|entry| {
            let status = entry.status();
            trace!(
                "{:?}: {:?}: status {:?}",
                repo.workdir(),
                entry.path(),
                status
            );
            status
        })
        .collect();

    let head = match repo.head() {
        Ok(h) => h,
        Err(e) => {
            debug!(
                "{}: SKIP NO HEAD (no commits in repo?)",
                repo.workdir().unwrap_or(repo.path()).display(),
            );
            trace!(
                "{}: {}",
                repo.workdir().unwrap_or(repo.path()).display(),
                e.to_string()
            );
            return Ok(GitCheckResult {
                state,
                statuses: vec![],
                commits_ahead: 0,
                commits_behind: 0,
            });
        }
    };

    let head_ref = head.resolve()?;

    let (commits_ahead, commits_behind) = if head_ref.is_branch() {
        // Obtain head object ID
        let head_oid = head_ref
            .target()
            .ok_or_else(|| -> ErrBox { format!("Failed to resolve HEAD ref").into() })?;

        let branch_name = head_ref
            .name()
            .ok_or_else(|| -> ErrBox { format!("Failed to get HEAD branch name").into() })?;
        if let Ok(remote_branch_name) = repo.branch_upstream_name(branch_name) {
            let remote_branch_name = remote_branch_name
                .as_str()
                .ok_or_else(|| -> ErrBox {
                    format!("Failed to decode remote branch name to UTF-8").into()
                })?
                .to_owned();

            let remote_oid = repo.refname_to_id(&remote_branch_name)?;

            repo.graph_ahead_behind(head_oid, remote_oid)?
        } else {
            debug!(
                "{}: SKIP NO REMOTE FOR {}",
                repo.workdir().unwrap_or(repo.path()).display(),
                branch_name
            );
            (0, 0)
        }
    } else {
        debug!("{}: SKIP HEAD DETACHED ", repo.path().display());
        (0, 0)
    };

    Ok(GitCheckResult {
        state,
        statuses,
        commits_ahead,
        commits_behind,
    })
}

pub fn find_git_repos_recursive<P: AsRef<Path>>(dir: P) -> Result<Vec<Repository>, ErrBox> {
    let mut stack = VecDeque::new();
    stack.push_back(dir.as_ref().to_owned());
    let mut found_repos = Vec::new();

    while let Some(cur_path) = stack.pop_front() {
        if cur_path.is_dir() {
            match Repository::discover(cur_path.clone()) {
                Ok(r) => {
                    trace!("HIT {}", cur_path.display());
                    found_repos.push(r);
                }
                Err(e) => {
                    trace!("MISS {} ERR {}", cur_path.display(), e.to_string());

                    let contents = fs::read_dir(cur_path)?
                        .map(|e_res| e_res.map(|e| e.path()))
                        .collect::<Result<Vec<_>, _>>()?;

                    stack.extend(contents);
                }
            }
        } else {
            trace!("MISS {} NON-DIR", cur_path.display());
        }
    }

    Ok(found_repos)
}
