use api::item::{Inbox, InboxItem};

use super::{Item, ItemState, ItemStore};
use crate::store::StoreError;

/// Open items grouped by who acts next, each group oldest first.
pub fn inbox(store: &impl ItemStore) -> Result<Inbox, StoreError> {
    let mut inbox = Inbox::default();
    for item in store.open_items(None)? {
        let group = match item.state {
            ItemState::NeedsReview => &mut inbox.needs_you,
            ItemState::ChangesRequested => &mut inbox.with_agents,
            ItemState::Merging => &mut inbox.merging,
            ItemState::Merged | ItemState::Abandoned => continue,
        };
        group.push(entry(&item));
    }
    Ok(inbox)
}

fn entry(item: &Item) -> InboxItem {
    InboxItem {
        id: item.id.to_string(),
        title: item.title.clone(),
        branch: item.branch.clone(),
        revision: item.revision.number,
        files: item.revision.stat.files,
        additions: item.revision.stat.additions,
        deletions: item.revision.stat.deletions,
        created_at: item.created_at.clone(),
        updated_at: item.revision.submitted_at.clone(),
    }
}

#[cfg(test)]
#[path = "inbox_tests.rs"]
mod tests;
