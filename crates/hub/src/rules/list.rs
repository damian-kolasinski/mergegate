//! `GET /api/projects/{name}/rules`: the applied rules and the changes
//! waiting for the operator.

use std::sync::Arc;

use axum::Json;
use axum::extract::rejection::PathRejection;
use axum::extract::{self, State};
use mergegate_core::item::ItemStore;
use mergegate_core::project::{self, ProjectName};
use mergegate_core::store::StoreError;
use mergegate_core::triage::{
    Kind, Preview, Proposal, ProposalState, Rule, RulesFile, RulesStore, Tier,
};
use store_sqlite::Store;

use super::watch::{RulesFiles, changed};
use crate::reply::Failure;
use crate::server::AppState;
use crate::{items, projects};

pub(super) async fn list(
    State(state): State<Arc<AppState>>,
    name: Result<extract::Path<String>, PathRejection>,
) -> Result<Json<api::rules::Rules>, Failure> {
    let extract::Path(name) = name?;
    let app = state.clone();
    let (rules, changes) = state
        .blocking(move |store, _| {
            let project = project::find(store, &name).map_err(projects::failure)?;
            let changes = app
                .rules
                .sync(store, &project.name)
                .map_err(|e| items::failure(e.into()))?;
            let rules =
                read(store, &app.rules, &project.name).map_err(|e| items::failure(e.into()))?;
            Ok::<_, Failure>((rules, changes))
        })
        .await?;
    changed(&state, &changes);
    Ok(Json(rules))
}

/// The answer to [`list`], and to a decision on a change.
pub(super) fn read(
    store: &Store,
    files: &RulesFiles,
    project: &ProjectName,
) -> Result<api::rules::Rules, StoreError> {
    let edit = match store.held_edit(project)? {
        Some(edit) => Some(api::rules::Edit {
            rules: edit.rules.rules().map(rule_to_api).collect(),
            preview: preview_to_api(store, &edit.preview)?,
        }),
        None => None,
    };
    Ok(api::rules::Rules {
        project: project.to_string(),
        location: files.file.location(project),
        rules: store
            .applied_rules(project)?
            .rules()
            .map(rule_to_api)
            .collect(),
        error: files.error(project),
        pending: store
            .pending_proposals(project)?
            .iter()
            .map(|p| proposal_to_api(store, p))
            .collect::<Result<_, _>>()?,
        edit,
    })
}

/// Proposals and a held edit waiting for the operator.
pub(crate) fn waiting(store: &Store, project: &ProjectName) -> Result<u32, StoreError> {
    let edit = store.held_edit(project)?.is_some();
    Ok(store.pending_proposals(project)?.len() as u32 + u32::from(edit))
}

pub(super) fn proposal_to_api(
    store: &Store,
    proposal: &Proposal,
) -> Result<api::rules::Proposal, StoreError> {
    Ok(api::rules::Proposal {
        rule: rule_to_api(&proposal.rule),
        state: match proposal.state {
            ProposalState::Pending => api::rules::ProposalState::Pending,
            ProposalState::Applied => api::rules::ProposalState::Applied,
            ProposalState::Rejected => api::rules::ProposalState::Rejected,
        },
        preview: match &proposal.preview {
            Some(preview) => Some(preview_to_api(store, preview)?),
            None => None,
        },
    })
}

fn preview_to_api(store: &Store, preview: &Preview) -> Result<api::rules::Preview, StoreError> {
    let hidden = preview
        .hidden
        .iter()
        .map(|file| {
            let item = store.item(&file.item)?;
            Ok(api::rules::HiddenFile {
                item: file.item.to_string(),
                name: item
                    .as_ref()
                    .map_or_else(|| file.item.clone(), |item| item.name())
                    .to_string(),
                title: item.map(|item| item.title).unwrap_or_default(),
                path: file.path.clone(),
                rule: file.rule.to_string(),
            })
        })
        .collect::<Result<_, StoreError>>()?;
    Ok(api::rules::Preview {
        items: preview.items,
        hidden,
    })
}

pub(super) fn rule_to_api(rule: &Rule) -> api::rules::Rule {
    api::rules::Rule {
        id: rule.id.to_string(),
        tier: tier_to_api(rule.tier),
        path: rule.path.clone(),
        kind: rule.kind.map(kind_to_api),
        max_lines: rule.max_lines,
        diff_matches: rule.diff_matches.clone(),
        reason: rule.reason.clone(),
        summary: rule.summary(),
    }
}

pub(crate) fn tier_to_api(tier: Tier) -> api::rules::Tier {
    match tier {
        Tier::Review => api::rules::Tier::Review,
        Tier::Trusted => api::rules::Tier::Trusted,
    }
}

pub(super) fn tier_from_api(tier: api::rules::Tier) -> Tier {
    match tier {
        api::rules::Tier::Review => Tier::Review,
        api::rules::Tier::Trusted => Tier::Trusted,
    }
}

fn kind_to_api(kind: Kind) -> api::rules::Kind {
    match kind {
        Kind::Added => api::rules::Kind::Added,
        Kind::Deleted => api::rules::Kind::Deleted,
        Kind::Renamed => api::rules::Kind::Renamed,
        Kind::Modified => api::rules::Kind::Modified,
        Kind::Generated => api::rules::Kind::Generated,
    }
}

pub(super) fn kind_from_api(kind: api::rules::Kind) -> Kind {
    match kind {
        api::rules::Kind::Added => Kind::Added,
        api::rules::Kind::Deleted => Kind::Deleted,
        api::rules::Kind::Renamed => Kind::Renamed,
        api::rules::Kind::Modified => Kind::Modified,
        api::rules::Kind::Generated => Kind::Generated,
    }
}
