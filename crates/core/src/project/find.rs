use std::path::Path;

use super::{Project, ProjectError, ProjectName, ProjectStore};
use crate::git::Git;

pub fn find(store: &impl ProjectStore, name: &str) -> Result<Project, ProjectError> {
    let not_found = || ProjectError::NotFound(name.into());
    let name = ProjectName::parse(name).map_err(|_| not_found())?;
    store.project(&name)?.ok_or_else(not_found)
}

/// The project whose repo contains `path`, from any of its worktrees.
pub fn at(store: &impl ProjectStore, git: &impl Git, path: &Path) -> Result<Project, ProjectError> {
    let repo = git.inspect(path)?;
    store
        .project_with_upstream(&repo.root)?
        .ok_or(ProjectError::NoProjectAt(repo.root))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::{FakeGit, FakeStore};
    use crate::project::{AddProject, add};

    #[test]
    fn finds_projects_by_name_and_by_path() {
        let git = FakeGit::default().repo("/src/app", &["main"], None).repo(
            "/src/other",
            &["main"],
            None,
        );
        let mut store = FakeStore::default();
        let request = AddProject {
            path: "/src/app".into(),
            ..AddProject::default()
        };
        add(&mut store, &git, request).unwrap();

        assert_eq!(find(&store, "app").unwrap().upstream, Path::new("/src/app"));
        assert!(matches!(find(&store, "nope"), Err(ProjectError::NotFound(n)) if n == "nope"));
        assert!(matches!(
            find(&store, "Not A Name"),
            Err(ProjectError::NotFound(_))
        ));

        assert_eq!(
            at(&store, &git, Path::new("/src/app/deep/dir"))
                .unwrap()
                .name
                .as_str(),
            "app"
        );
        assert!(matches!(
            at(&store, &git, Path::new("/src/other")),
            Err(ProjectError::NoProjectAt(_))
        ));
        assert!(matches!(
            at(&store, &git, Path::new("/tmp")),
            Err(ProjectError::NotARepo(_))
        ));
    }
}
