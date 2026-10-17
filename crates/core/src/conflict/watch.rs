//! Open items test-merged into their target as it moves, so a conflict
//! reaches the agent before the operator gets to the item.
//!
//! A pass reads under the store, fetches and merges without it, and hands
//! back under it again, so a slow fetch never holds up the hub's requests.

use std::collections::BTreeSet;

use super::{ConflictError, ConflictGit};
use crate::feedback::{EventKind, FeedbackStore, Notice, NoticeKind, Source};
use crate::git::Upstream;
use crate::github::{Github, GithubRepo};
use crate::item::{Item, ItemBackend, ItemId, ItemState, ItemStore};
use crate::project::{Project, ProjectName, ProjectStore};
use crate::store::StoreError;

/// What a project's last pass tried, as item number, target commit and
/// head, so a pass where nothing moved merges nothing. It holds only what
/// the last pass saw, so it never outgrows the open items.
#[derive(Debug, Default)]
pub struct Seen(BTreeSet<(u64, String, String)>);

/// The open items of a project to test-merge, grouped by where their target
/// comes from, so each is fetched once however many items use it. Items in
/// Merging are left to their merge, and one whose agent already heard of a
/// conflict on its revision isn't tried again, though its target is fetched.
///
/// Nor is an item stacked on an open one: its branch carries its
/// parent's commits, whose conflict is the parent's to fix, and rebasing
/// onto the target would take it off its parent. Its own diff is tried when
/// its parent lands.
#[derive(Debug)]
pub struct Watch {
    project: Project,
    sources: Vec<(Option<GithubRepo>, String, Vec<Item>)>,
}

/// What a pass found without the store.
#[derive(Debug)]
pub struct Merges {
    project: ProjectName,
    /// Each target fetched, at the commit fetched.
    fetched: Vec<(String, String)>,
    /// Each item that conflicts, as tried, with its notice.
    found: Vec<(Item, Notice)>,
    /// Why a target or an item couldn't be tried. The rest still were.
    pub errors: Vec<ConflictError>,
    /// What the next pass skips, once these are handed back.
    pub seen: Seen,
}

impl Watch {
    /// Reads, under the store, what to try. None when the project is gone.
    pub fn read<S: ProjectStore + ItemStore + FeedbackStore>(
        store: &S,
        project: &ProjectName,
    ) -> Result<Option<Self>, ConflictError> {
        let Some(project) = store.project(project)? else {
            return Ok(None);
        };
        let mut sources: Vec<(Option<GithubRepo>, String, Vec<Item>)> = Vec::new();
        let open = store.open_items(Some(&project.name))?;
        for item in &open {
            if item.state == ItemState::Merging {
                continue;
            }
            let repo = match &item.backend {
                ItemBackend::Github { repo, .. } => Some(repo.clone()),
                ItemBackend::Local => None,
            };
            let index = match sources
                .iter()
                .position(|(r, target, _)| *r == repo && *target == item.target)
            {
                Some(index) => index,
                None => {
                    sources.push((repo, item.target.clone(), Vec::new()));
                    sources.len() - 1
                }
            };
            // A told item's target is still fetched, so staging follows it
            // for the rebase the agent was told to do. So is a stacked one's:
            // its own diff is tried as its parent lands, and once its parent
            // closed its agent was told to rebase off it.
            if item.parent.is_none() && !told(store, item)? {
                sources[index].2.push(item.clone());
            }
        }
        Ok(Some(Self { project, sources }))
    }

    /// Fetches each target and test-merges its items into it, skipping what
    /// the last pass already tried at the same commits. Needs no store: the
    /// fetch writes only the watch's own refs.
    pub fn try_merges(&self, git: &impl ConflictGit, github: &impl Github, seen: &Seen) -> Merges {
        let project = &self.project.name;
        let mut merges = Merges {
            project: project.clone(),
            fetched: Vec::new(),
            found: Vec::new(),
            errors: Vec::new(),
            seen: Seen::default(),
        };
        for (repo, target, items) in &self.sources {
            let fetched = (|| {
                let upstream = match repo {
                    Some(repo) => Upstream::Remote(github.remote(repo)?),
                    None => Upstream::Repo(self.project.upstream.clone()),
                };
                Ok::<_, ConflictError>(git.fetch_watched(project, &upstream, target)?)
            })();
            let target_commit = match fetched {
                Ok(commit) => commit,
                Err(e) => {
                    merges.errors.push(e);
                    continue;
                }
            };
            merges.fetched.push((target.clone(), target_commit.clone()));
            for item in items {
                let key = (
                    item.id.number(),
                    target_commit.clone(),
                    item.revision.head.clone(),
                );
                if !seen.0.contains(&key) {
                    match git.conflicts(project, &target_commit, &item.revision.head) {
                        Ok(paths) if paths.is_empty() => {}
                        Ok(paths) => merges.found.push((
                            item.clone(),
                            Notice {
                                revision: item.revision.number,
                                source: Source::Operator,
                                kind: NoticeKind::MergeConflict {
                                    target_commit: target_commit.clone(),
                                    paths,
                                },
                            },
                        )),
                        Err(e) => {
                            merges.errors.push(e.into());
                            continue;
                        }
                    }
                }
                merges.seen.0.insert(key);
            }
        }
        merges
    }
}

impl Merges {
    /// Under the store again: moves staging's targets up to what was
    /// fetched, so an agent told to rebase fetches what it conflicts with,
    /// and hands each conflicting item back to its agent with
    /// `merge.conflict`, as a merge that conflicts does. An item that moved
    /// on meanwhile (a new revision, Merging, closed, or told by its merge)
    /// is left alone. Returns the items handed back.
    pub fn hand_back<S: ItemStore + FeedbackStore>(
        &self,
        store: &mut S,
        git: &impl ConflictGit,
    ) -> Result<Vec<ItemId>, ConflictError> {
        for (target, commit) in &self.fetched {
            git.advance(&self.project, target, commit)?;
        }
        let mut handed_back = Vec::new();
        for (tried, notice) in &self.found {
            let Some(item) = store.item(&tried.id)? else {
                continue;
            };
            let unmoved = item.state.is_open()
                && item.state != ItemState::Merging
                && item.revision.number == tried.revision.number
                && item.revision.head == tried.revision.head;
            if !unmoved || told(store, &item)? {
                continue;
            }
            store.notify(&item.id, ItemState::ChangesRequested, notice.clone())?;
            handed_back.push(item.id);
        }
        Ok(handed_back)
    }
}

/// Whether the item's agent already heard of a conflict on this revision,
/// from an earlier pass or a merge.
fn told(store: &impl FeedbackStore, item: &Item) -> Result<bool, StoreError> {
    Ok(store.events(&item.id, 0)?.iter().any(|event| {
        matches!(
            &event.kind,
            EventKind::Notice(Notice {
                revision,
                kind: NoticeKind::MergeConflict { .. },
                ..
            }) if *revision == item.revision.number
        )
    }))
}

#[cfg(test)]
#[path = "watch_tests.rs"]
mod tests;
