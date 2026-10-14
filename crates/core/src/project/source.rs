use super::{Backend, Project, ProjectError};
use crate::git::Upstream;
use crate::github::Github;

/// Where staging takes the target from for `backend`'s items: the
/// operator's repo for `local`, GitHub for `github`.
pub fn fetch_from(
    project: &Project,
    backend: Backend,
    github: &impl Github,
) -> Result<Upstream, ProjectError> {
    match backend {
        Backend::Local => Ok(Upstream::Repo(project.upstream.clone())),
        Backend::Github => {
            let repo = project
                .github
                .as_ref()
                .ok_or_else(|| ProjectError::NoGithubSet(project.name.clone()))?;
            Ok(Upstream::Remote(github.remote(repo)?))
        }
    }
}
