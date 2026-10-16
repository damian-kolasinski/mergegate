//! Registering pushed work as an item or a new revision (F4).

use super::diff_files::diff_files;
use super::priority::note_of;
use super::publish::{Revision, describe, description, publish};
use super::stack::{self, StackError, pull_base};
use super::{
    Item, ItemBackend, ItemError, ItemGit, ItemState, ItemStore, NewItem, NewRevision, On,
    Priority, valid_branch, valid_commit,
};
use crate::agent::{self, AgentStore};
use crate::diff::{DiffGit, DiffStore};
use crate::feedback::FeedbackStore;
use crate::git::Upstream;
use crate::github::Github;
use crate::interdiff::{InterdiffGit, Recorded, record};
use crate::project::{Backend, ProjectError, ProjectStore, find};
use crate::review::{CommentId, ReviewStore};
use crate::store::StoreError;
use crate::triage::{Guide, RulesStore, TriageGit, triage_files};
use crate::viewed::ViewedStore;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Submit {
    pub project: String,
    pub branch: String,
    pub head: String,
    pub title: Option<String>,
    /// The submitting worktree's path, which `mg resume` names.
    pub worktree: Option<String>,
    /// The command that wakes the item's agent; empty removes it. Kept when
    /// none is sent.
    pub wake: Option<String>,
    /// The agent's review guide (F11); blank removes it. The last one is
    /// kept when none is sent.
    pub guide: Option<String>,
    /// The item's priority and note from the agent's brief (§10). Only the
    /// submit that opens the item sets them, so an agent repeating its brief
    /// can't undo a change the operator made since.
    pub priority: Option<Priority>,
    pub note: Option<String>,
    /// What to stack the item on (§5.1); none keeps what it's on.
    pub on: Option<On>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Submitted {
    pub item: Item,
    pub new_revision: bool,
    /// The submit gave an existing item a new title, which a merge would
    /// land as its commit message.
    pub retitled: bool,
    /// What the agent should fix in the revision's guide, and a priority or
    /// note it sent that the item ignored.
    pub warnings: Vec<String>,
}

/// The branch must already be pushed to staging at `request.head`. The
/// target is fetched fresh first, so the merge-base is current. Every file's
/// diff, and the interdiff from each earlier revision, is cached and the file
/// triaged by the applied rules before the revision is recorded. A submit at
/// the open item's head still records its worktree, wake, guide and title.
///
/// A `github` item is published before it's recorded: its branch pushed and
/// its pull request opened or updated, described by the item's ID and guide.
/// If GitHub fails, the submit fails and nothing is recorded.
///
/// A stacked item's revision is diffed against the newest parent revision
/// its head contains, and its pull request is based on the parent's branch.
/// The item stacked on this one hears of its new revision.
pub fn submit<
    S: ProjectStore
        + ItemStore
        + DiffStore
        + ViewedStore
        + AgentStore
        + RulesStore
        + ReviewStore
        + FeedbackStore,
>(
    store: &mut S,
    git: &(impl ItemGit + DiffGit + InterdiffGit + TriageGit),
    github: &impl Github,
    request: Submit,
) -> Result<Submitted, ItemError> {
    let project = find(store, &request.project)?;
    let Submit {
        branch,
        head,
        title,
        worktree,
        wake,
        guide,
        priority,
        note,
        on,
        ..
    } = request;
    let guide = guide.map(|text| (!text.trim().is_empty()).then_some(text));
    let note = note_of(note);
    let prioritized = priority.is_some() || note.is_some();
    if !valid_branch(&branch) {
        return Err(ItemError::InvalidBranch(branch));
    }
    if !valid_commit(&head) {
        return Err(ItemError::InvalidCommit(head));
    }
    let existing = store.open_item(&project.name, &branch)?;
    let restacking = on.is_some();
    let parent = stack::parent_of(store, &project.name, existing.as_ref(), on)?;
    let target = match (&existing, &parent) {
        (Some(item), _) => &item.target,
        (None, Some(parent)) => &parent.target,
        (None, None) => &project.target,
    }
    .clone();
    if branch == target || branch == project.target {
        return Err(ItemError::TargetBranch { branch });
    }
    if let Some(parent) = parent.as_ref().filter(|parent| parent.branch == branch) {
        return Err(StackError::OnItself(parent.id.clone()).into());
    }
    // Moving the item onto another parent at the same head is a new
    // revision: its diff is against another base.
    let restacked = existing
        .as_ref()
        .is_some_and(|item| restacking && item.parent.as_ref() != parent.as_ref().map(|p| &p.id));
    if let Some(item) = existing
        .as_ref()
        .filter(|i| i.revision.head == head && !restacked)
    {
        // Pinning again repairs a submit that failed after recording.
        git.pin(&project.name, &item.pin_ref(), &head)?;
        let mut item = item.clone();
        let number = item.revision.number;
        let retitle = title
            .as_deref()
            .map(str::trim)
            .filter(|title| !title.is_empty() && *title != item.title);
        let described = store.guide(&item.id, number)?;
        let reguide = guide.as_ref().filter(|guide| **guide != described);
        // GitHub first, as for a new revision, so a failure records nothing.
        if let ItemBackend::Github { repo, pull } = &item.backend
            && (retitle.is_some() || reguide.is_some())
        {
            let title = retitle.unwrap_or(&item.title);
            let body = reguide.map(|guide| description(&item.id, guide.as_deref()));
            describe(github, repo, pull.number, title, body.as_deref())?;
        }
        if let Some(title) = retitle {
            store.set_title(&item.id, title)?;
            item.title = title.to_owned();
        }
        agent::record(store, &item.id, worktree, wake)?;
        let guide = match guide {
            Some(guide) => {
                store.set_guide(&item.id, number, guide.as_deref())?;
                guide
            }
            None => described,
        };
        let files = store.revision_files(&item.id, number)?;
        let mut warnings = warnings(store, Some(&item), number, guide.as_deref(), &files)?;
        warnings.extend(prioritized.then(|| kept(&item)));
        return Ok(Submitted {
            item,
            new_revision: false,
            retitled: retitle.is_some(),
            warnings,
        });
    }
    match git.branch_head(&project.name, &branch)? {
        None => return Err(ItemError::NoBranch(branch)),
        Some(found) if found != head => {
            return Err(ItemError::NotPushed {
                branch,
                head,
                found,
            });
        }
        Some(_) => {}
    }
    // An open item stays where it was opened, whatever the project says now,
    // and a new one stacked on another goes where its parent went.
    let opened = existing.as_ref().or(parent.as_ref());
    let github_repo = match opened.map(|item| &item.backend) {
        Some(ItemBackend::Github { repo, .. }) => Some(repo.clone()),
        Some(ItemBackend::Local) => None,
        None => match project.backend {
            Backend::Github => Some(
                project
                    .github
                    .clone()
                    .ok_or_else(|| ProjectError::NoGithubSet(project.name.clone()))?,
            ),
            Backend::Local => None,
        },
    };
    let upstream = match &github_repo {
        Some(repo) => Upstream::Remote(github.remote(repo)?),
        None => Upstream::Repo(project.upstream.clone()),
    };
    git.fetch_target(&project.name, &upstream, &target)?;
    let base = match &parent {
        Some(parent) => {
            let base = stack::base(store, git, parent, &branch, &head)?;
            if base == head {
                return Err(ItemError::NothingToSubmit {
                    branch,
                    target: parent.branch.clone(),
                });
            }
            base
        }
        None => match git.merge_base(&project.name, &head, &target)? {
            Some(base) if base == head => {
                return Err(ItemError::NothingToSubmit { branch, target });
            }
            Some(base) => base,
            None => return Err(ItemError::Unrelated { branch, target }),
        },
    };
    let files = git.changed_files(&project.name, &base, &head)?;
    let mut files = diff_files(store, git, &project.name, files)?;
    // A rename counts as generated only when both its paths are, as code
    // moved into a generated directory isn't.
    let paths: Vec<&str> = files
        .iter()
        .flat_map(|f| std::iter::once(&f.file.path).chain(&f.file.old_path))
        .map(String::as_str)
        .collect();
    // The base's attributes, so a revision can't mark its own files
    // generated; the guard keeps a changed `.gitattributes` reviewed too.
    let mut generated = git.generated(&project.name, &base, &paths)?.into_iter();
    for file in &mut files {
        let renamed = file.file.old_path.is_some();
        let new = generated.next().unwrap_or(false);
        let old = !renamed || generated.next().unwrap_or(false);
        file.generated = new && old;
    }
    let rules = store.applied_rules(&project.name)?;
    triage_files(store, &rules, &mut files)?;
    let Recorded { interdiffs, viewed } = match &existing {
        Some(item) => record(store, git, item, &head, &base, &files)?,
        None => Recorded::default(),
    };
    let title = match (title.as_deref().map(str::trim), &existing) {
        (Some(title), _) if !title.is_empty() => title.to_owned(),
        (_, Some(item)) => item.title.clone(),
        _ => git
            .first_subject(&project.name, &base, &head)?
            .filter(|subject| !subject.trim().is_empty())
            .unwrap_or_else(|| branch.clone()),
    };
    let described = match &existing {
        Some(item) => store.guide(&item.id, item.revision.number)?,
        None => None,
    };
    let guide = guide.unwrap_or_else(|| described.clone());
    let number = existing.as_ref().map_or(1, |item| item.revision.number + 1);
    // Before publishing, so a store error can't leave GitHub ahead of the hub.
    let mut warnings = warnings(store, existing.as_ref(), number, guide.as_deref(), &files)?;
    let backend = match (&upstream, github_repo) {
        (Upstream::Remote(remote), Some(repo)) => {
            let id = match &existing {
                Some(item) => item.id.clone(),
                None => store.next_id(&project.name)?,
            };
            let pull = publish(
                git,
                github,
                &project.name,
                (&repo, remote),
                existing.as_ref().map(|item| (item, described.as_deref())),
                Revision {
                    id: &id,
                    branch: &branch,
                    head: &head,
                    base: pull_base(&target, parent.as_ref()),
                    // Stacked now or before: its parent may have moved,
                    // merged or been abandoned since.
                    restack: parent.is_some()
                        || existing.as_ref().is_some_and(|item| item.parent.is_some()),
                    title: &title,
                    guide: guide.as_deref(),
                },
            )?;
            if let Some(left) = left_abandoned(store, existing.as_ref(), parent.as_ref())? {
                // Its branch stayed only for this pull request to be based on.
                let _ = github.delete_branch(&repo, &left.branch);
            }
            ItemBackend::Github { repo, pull }
        }
        _ => ItemBackend::Local,
    };
    if let Some(item) = existing.as_ref().filter(|_| prioritized) {
        warnings.push(kept(item));
    }
    let revision = NewRevision {
        head,
        base,
        files,
        guide,
        interdiffs,
        viewed,
        parent: parent.map(|parent| parent.id),
    };
    let retitled = existing.as_ref().is_some_and(|item| item.title != title);
    let item = match existing {
        Some(item) => store.add_revision(&item.id, &title, ItemState::NeedsReview, revision)?,
        None => store.insert_item(NewItem {
            project: project.name.clone(),
            branch,
            title,
            state: ItemState::NeedsReview,
            backend,
            target,
            revision,
            priority,
            note,
        })?,
    };
    git.pin(&project.name, &item.pin_ref(), &item.revision.head)?;
    agent::record(store, &item.id, worktree, wake)?;
    stack::updated(store, &item)?;
    Ok(Submitted {
        item,
        new_revision: true,
        retitled,
        warnings,
    })
}

/// The abandoned parent `existing` moves off, if it was on one.
fn left_abandoned(
    store: &impl ItemStore,
    existing: Option<&Item>,
    parent: Option<&Item>,
) -> Result<Option<Item>, StoreError> {
    let Some(old) = existing.and_then(|item| item.parent.as_ref()) else {
        return Ok(None);
    };
    if parent.is_some_and(|parent| parent.id == *old) {
        return Ok(None);
    }
    Ok(store
        .item(old)?
        .filter(|old| old.state == ItemState::Abandoned))
}

fn kept(item: &Item) -> String {
    let priority = item.priority.map_or("no priority", Priority::as_str);
    format!(
        "--priority and --note set only a new item's; {} keeps {priority}, which mg prioritize changes",
        item.id
    )
}

/// What the agent should fix in the guide of revision `revision`, of `item`
/// or of a new item when none.
fn warnings(
    store: &impl ReviewStore,
    item: Option<&Item>,
    revision: u32,
    guide: Option<&str>,
    files: &[super::FileStat],
) -> Result<Vec<String>, StoreError> {
    let Some(text) = guide else {
        return Ok(Vec::new());
    };
    let mut warnings = Guide::parse(text).warnings(text, files);
    let comments: Vec<CommentId> = match item {
        // A draft's ID is the operator's until a review sends it.
        Some(item) => store
            .comments(&item.id)?
            .into_iter()
            .filter(|comment| !comment.is_draft())
            .map(|comment| comment.id)
            .collect(),
        None => Vec::new(),
    };
    warnings.extend(Guide::review_warning(text, &comments, revision));
    Ok(warnings)
}

#[cfg(test)]
#[path = "submit_tests.rs"]
mod tests;
