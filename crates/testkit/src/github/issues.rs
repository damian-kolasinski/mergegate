use serde_json::{Value, json};

use super::server::Response;
use super::{Shared, State};

#[derive(Debug, Clone)]
pub struct Issue {
    pub repo: String,
    pub number: u64,
    pub title: String,
    pub body: String,
}

pub(super) fn list(state: &State, repo: &str, web: &str) -> Response {
    if state.issues_hidden {
        return Response::json(200, &json!([]));
    }
    let issues = state
        .issues
        .iter()
        .filter(|i| i.repo == repo)
        .map(|i| value(i, web))
        .collect::<Vec<_>>();
    Response::json(200, &json!(issues))
}

pub(super) fn create(shared: &Shared, state: &mut State, repo: &str, body: &Value) -> Response {
    if state.issue_post_unauthorized {
        return Response::json(401, &json!({"message": "Bad credentials"}));
    }
    if let Some(reason) = &state.refusal {
        return Response::json(403, &json!({"message": reason}));
    }
    let issue = Issue {
        repo: repo.into(),
        number: state.issues.len() as u64 + 1,
        title: body["title"].as_str().unwrap_or_default().into(),
        body: body["body"].as_str().unwrap_or_default().into(),
    };
    let response = Response::json(201, &value(&issue, &shared.web));
    state.issues.push(issue);
    response
}

fn value(issue: &Issue, web: &str) -> Value {
    json!({"number": issue.number, "title": issue.title, "body": issue.body, "html_url": format!("{web}/{}/issues/{}", issue.repo, issue.number)})
}
