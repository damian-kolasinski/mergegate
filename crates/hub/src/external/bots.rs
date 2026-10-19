//! Review bots' say on an item's pull request, read from their reactions on
//! its description with each poll. Held in memory only: after a restart the
//! first poll brings it back.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use api::event::UiEvent;
use mergegate_core::external;
use mergegate_core::github::{BotReview, BotReviewState, Github, bot_reviews};
use mergegate_core::item::{ItemError, ItemId, ItemStore};

use crate::server::AppState;

#[derive(Default)]
pub(crate) struct Bots {
    inner: Mutex<HashMap<ItemId, Vec<BotReview>>>,
}

impl Bots {
    pub(crate) fn of(&self, id: &ItemId) -> Vec<BotReview> {
        self.lock().get(id).cloned().unwrap_or_default()
    }

    /// Whether that changed what's held for `id`.
    fn set(&self, id: &ItemId, reviews: Vec<BotReview>) -> bool {
        let mut inner = self.lock();
        if inner.get(id).map_or(&[][..], Vec::as_slice) == reviews.as_slice() {
            return false;
        }
        if reviews.is_empty() {
            inner.remove(id);
        } else {
            inner.insert(id.clone(), reviews);
        }
        true
    }

    /// Forgets items no longer polled, which closed.
    pub(super) fn retain(&self, polled: &[ItemId]) {
        self.lock().retain(|id, _| polled.contains(id));
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<ItemId, Vec<BotReview>>> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Reads the reactions on `id`'s pull request with the store unlocked, and
/// tells open UIs when the bots' say changed.
pub(super) async fn watch(state: &Arc<AppState>, id: &ItemId) -> Result<(), ItemError> {
    let pull = {
        let id = id.clone();
        state
            .blocking(move |store, _| {
                let item = store
                    .item(&id)?
                    .ok_or_else(|| ItemError::NoItem(id.clone()))?;
                let pull = external::pull_of(&item).filter(|_| item.state.is_open());
                // The hub pushes each revision to the pull request as it's
                // submitted, so that is when its commits reached GitHub.
                let pushed = item.revision.submitted_at.clone();
                Ok::<_, ItemError>(pull.map(|(repo, number)| (repo.clone(), number, pushed)))
            })
            .await?
    };
    let reviews = match pull {
        Some((repo, number, pushed)) => {
            let reactions = state
                .github(move |github| github.pull_reactions(&repo, number))
                .await?;
            bot_reviews(&reactions, &pushed)
        }
        None => Vec::new(),
    };
    if state.bots.set(id, reviews) {
        state.events.send(UiEvent::ItemChanged {
            item: id.to_string(),
        });
    }
    Ok(())
}

pub(crate) fn to_api(reviews: &[BotReview]) -> Vec<api::bot::BotReview> {
    reviews
        .iter()
        .map(|review| api::bot::BotReview {
            login: review.login.clone(),
            name: review.name.clone(),
            state: match review.state {
                BotReviewState::Reviewing => api::bot::BotReviewState::Reviewing,
                BotReviewState::Approved => api::bot::BotReviewState::Approved,
            },
        })
        .collect()
}
