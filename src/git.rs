use failure::Error;
use git2::{Repository, RepositoryState, Status, StatusOptions};

pub fn check_repo(repo: &Repository) -> Result<GitCheckResult, Error> {
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

    Ok(GitCheckResult {
        unclean_state: if let RepositoryState::Clean = state {
            None
        } else {
            Some(state)
        },
        statuses,
    })
}

/// Encapsulates different things that can fail when checking a repo.
#[derive(Clone, Debug)]
pub struct GitCheckResult {
    pub unclean_state: Option<RepositoryState>,
    pub statuses: Vec<Status>,
}

impl GitCheckResult {
    pub fn is_all_good(&self) -> bool {
        self.unclean_state.is_none() && self.statuses.is_empty()
    }

    /// Returns a list of facts about the result.
    pub fn describe(&self) -> Vec<String> {
        if self.is_all_good() {
            return vec!["all good".to_owned()];
        }

        let mut ret = Vec::new();

        if let Some(state) = self.unclean_state {
            ret.push(format!("unclean state {:?}", state));
        }

        let change_count = self.statuses.len();
        if change_count > 0 {
            ret.push(format!(
                "{} uncommitted change{}",
                change_count,
                if change_count > 1 { "s" } else { "" }
            ));
        }

        ret
    }
}
