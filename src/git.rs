//! Git directory checking helpers

use failure::{format_err, Error};
use git2::{Repository, RepositoryState, Status, StatusOptions};

use std::path::Path;

pub fn check_repo_in_dir<P: AsRef<Path>>(dir: P) -> Result<GitCheckResult, Error> {
    let repo = Repository::discover(dir.as_ref())?;

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

    let head_ref = repo
        .head()
        .map_err(|e| {
            format_err!(
                "{}: Could not obtain HEAD. Does this repo have any commits? ({})",
                dir.as_ref().to_str().unwrap_or("<not utf-8>"),
		e
            )
        })?
        .resolve()?;

    let (commits_ahead, commits_behind) = if head_ref.is_branch() {
        // Obtain head object ID
        let head_oid = head_ref
            .target()
            .ok_or_else(|| format_err!("Failed to resolve HEAD ref"))?;

        let branch_name = head_ref
            .name()
            .ok_or_else(|| format_err!("Failed to get HEAD branch name"))?;
        if let Ok(remote_branch_name) = repo.branch_upstream_name(branch_name) {
            let remote_branch_name = remote_branch_name
                .as_str()
                .ok_or_else(|| format_err!("Failed to decode remote branch name to UTF-8"))?
                .to_owned();

            let remote_oid = repo.refname_to_id(&remote_branch_name)?;

            repo.graph_ahead_behind(head_oid, remote_oid)?
        } else {
            info!(
                "{}: {} does not appear to belong to a remote, skipping ahead/behind...",
                dir.as_ref().to_str().unwrap_or("<not utf-8>"),
                branch_name
            );
            (0, 0)
        }
    } else {
        debug!(
            "{}: HEAD is not a branch, skipping ahead/behind...",
            dir.as_ref().to_str().unwrap_or("<not utf-8>")
        );
        (0, 0)
    };

    Ok(GitCheckResult {
        state,
        statuses,
        commits_ahead,
        commits_behind,
    })
}

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
