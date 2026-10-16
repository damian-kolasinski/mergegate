//! The REST endpoints the hub calls, answering as GitHub does, errors
//! included.

use serde_json::{Value, json};

use super::comments::{self, CommentKind};
use super::server::{Request, Response};
use super::{Pull, PullState, Shared, State};

pub(super) fn handle(shared: &Shared, request: &Request, path: &str) -> Response {
    let mut state = shared.state();
    let expected = format!("Bearer {}", state.token);
    if state.token.is_empty() || request.header("authorization") != Some(expected.as_str()) {
        return message(401, "Bad credentials");
    }
    let body: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
    let segments: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let method = request.method.as_str();
    if method == "GET" && segments == ["user"] {
        return Response::json(200, &json!({ "login": state.viewer }));
    }
    let ["repos", owner, name, rest @ ..] = segments.as_slice() else {
        return message(404, "Not Found");
    };
    let repo = format!("{owner}/{name}");
    let Some(can_push) = shared.can_push(&state, &repo) else {
        return message(404, "Not Found");
    };
    let call = Call {
        shared,
        repo,
        can_push,
    };
    match (method, rest) {
        ("GET", []) => call.repo(),
        ("GET", ["pulls"]) => call.find(&state, request),
        ("POST", ["pulls"]) => call.open(&mut state, &body),
        ("GET", ["pulls", number]) => call.with_pull(&mut state, number, |call, pull| {
            Response::json(200, &call.pull_json(pull))
        }),
        ("PATCH", ["pulls", number]) => call.update(&mut state, number, &body),
        ("PUT", ["pulls", number, "merge"]) => call.merge(&mut state, number, &body),
        ("GET", ["pulls", "comments", id]) => {
            comments::one(shared, &state, &call.repo, None, CommentKind::Inline, id)
        }
        ("GET", ["issues", "comments", id]) => {
            comments::one(shared, &state, &call.repo, None, CommentKind::Issue, id)
        }
        ("GET", ["pulls", number, "reviews", id]) => comments::one(
            shared,
            &state,
            &call.repo,
            Some(number),
            CommentKind::Review,
            id,
        ),
        ("GET", [list, number, which]) => {
            let kind = match (*list, *which) {
                ("pulls", "comments") => CommentKind::Inline,
                ("issues", "comments") => CommentKind::Issue,
                ("pulls", "reviews") => CommentKind::Review,
                _ => return message(404, "Not Found"),
            };
            let head = shared.pull_head(&state, &call.repo, number.parse().unwrap_or(0));
            comments::list(shared, &mut state, request, &call.repo, number, kind, head)
        }
        ("DELETE", ["git", "refs", "heads", branch @ ..]) => {
            call.delete_branch(&mut state, &branch.join("/"))
        }
        _ => message(404, "Not Found"),
    }
}

struct Call<'a> {
    shared: &'a Shared,
    repo: String,
    can_push: bool,
}

impl Call<'_> {
    fn repo(&self) -> Response {
        Response::json(
            200,
            &json!({
                "full_name": self.repo,
                "permissions": { "pull": true, "push": self.can_push, "admin": false },
            }),
        )
    }

    fn find(&self, state: &State, request: &Request) -> Response {
        let head = request.param("head").unwrap_or_default();
        let base = request.param("base").unwrap_or_default();
        let open = request.param("state").as_deref() == Some("open");
        let pulls: Vec<Value> = state
            .pulls
            .iter()
            .filter(|p| p.repo == self.repo)
            .filter(|p| head.is_empty() || head == format!("{}:{}", self.owner(), p.branch))
            .filter(|p| base.is_empty() || base == p.base)
            .filter(|p| !open || p.state == PullState::Open)
            .map(|p| self.pull_json(p))
            .collect();
        Response::json(200, &Value::Array(pulls))
    }

    fn open(&self, state: &mut State, body: &Value) -> Response {
        if !self.can_push {
            return message(403, "Resource not accessible by integration");
        }
        let text = |key: &str| body[key].as_str().unwrap_or_default().to_owned();
        let (branch, base) = (text("head"), text("base"));
        for (field, branch) in [("head", &branch), ("base", &base)] {
            if self.head_of(branch).is_none() {
                return invalid(&format!("{field} {branch} doesn't exist"));
            }
        }
        let taken = state
            .pulls
            .iter()
            .any(|p| p.repo == self.repo && p.branch == branch && p.state == PullState::Open);
        if taken {
            return invalid(&format!(
                "A pull request already exists for {}:{branch}.",
                self.owner()
            ));
        }
        let elsewhere = state.opened_elsewhere.get(&self.repo).copied();
        let number = state.pulls.iter().filter(|p| p.repo == self.repo).count() as u64
            + elsewhere.unwrap_or(0)
            + 1;
        let pull = Pull {
            repo: self.repo.clone(),
            number,
            branch,
            base,
            title: text("title"),
            body: text("body"),
            draft: body["draft"] == true,
            state: PullState::Open,
        };
        let response = Response::json(201, &self.pull_json(&pull));
        state.pulls.push(pull);
        response
    }

    fn update(&self, state: &mut State, number: &str, body: &Value) -> Response {
        if !self.can_push {
            return message(403, "Resource not accessible by integration");
        }
        self.with_pull(state, number, |call, pull| {
            if let Some(base) = body["base"].as_str() {
                if call.head_of(base).is_none() {
                    return invalid(&format!("base {base} doesn't exist"));
                }
                pull.base = base.into();
            }
            if let Some(title) = body["title"].as_str() {
                pull.title = title.into();
            }
            if let Some(text) = body["body"].as_str() {
                pull.body = text.into();
            }
            if body["state"] == "closed" && pull.state == PullState::Open {
                pull.state = PullState::Closed;
            }
            Response::json(200, &call.pull_json(pull))
        })
    }

    /// A squash merge: one commit on the base with the branch's tree merged
    /// in, as the signed-in account, committed by GitHub.
    fn merge(&self, state: &mut State, number: &str, body: &Value) -> Response {
        if !self.can_push {
            return message(403, "Resource not accessible by integration");
        }
        if let Some(reason) = state.refusal.clone() {
            return message(405, &reason);
        }
        self.with_pull(state, number, |call, pull| {
            if pull.state != PullState::Open {
                return message(405, "Pull Request is not mergeable");
            }
            let head = call.head_of(&pull.branch).unwrap_or_default();
            if body["sha"].as_str().is_some_and(|sha| sha != head) {
                return message(409, "Head branch was modified. Review and try the merge again.");
            }
            let base = call.head_of(&pull.base).unwrap_or_default();
            let tree = call.bare(&["merge-tree", "--write-tree", &base, &head], &[]);
            let Some(tree) = tree else {
                return message(405, "Pull Request is not mergeable");
            };
            let title = body["commit_title"]
                .as_str()
                .map_or_else(|| format!("{} (#{})", pull.title, pull.number), Into::into);
            let mut text = title;
            if let Some(extra) = body["commit_message"].as_str().filter(|m| !m.is_empty()) {
                text = format!("{text}\n\n{extra}");
            }
            let identity = [
                ("GIT_AUTHOR_NAME", "Operator"),
                ("GIT_AUTHOR_EMAIL", "operator@users.noreply.github.com"),
                ("GIT_COMMITTER_NAME", "GitHub"),
                ("GIT_COMMITTER_EMAIL", "noreply@github.com"),
            ];
            let commit = call
                .bare(&["commit-tree", &tree, "-p", &base, "-m", &text], &identity)
                .expect("commit the squash");
            let base_ref = format!("refs/heads/{}", pull.base);
            call.bare(&["update-ref", &base_ref, &commit, &base], &[])
                .expect("move the base");
            pull.state = PullState::Merged {
                head,
                commit: commit.clone(),
            };
            Response::json(
                200,
                &json!({ "sha": commit, "merged": true, "message": "Pull Request successfully merged" }),
            )
        })
    }

    /// As GitHub does, it closes the open pull requests based on the branch,
    /// which then can't be retargeted or reopened.
    fn delete_branch(&self, state: &mut State, branch: &str) -> Response {
        if !self.can_push {
            return message(403, "Resource not accessible by integration");
        }
        if self.head_of(branch).is_none() {
            return message(422, "Reference does not exist");
        }
        self.bare(&["update-ref", "-d", &format!("refs/heads/{branch}")], &[])
            .expect("delete the branch");
        for pull in state
            .pulls
            .iter_mut()
            .filter(|p| p.repo == self.repo && p.base == branch && p.state == PullState::Open)
        {
            pull.state = PullState::Closed;
        }
        Response::empty(204)
    }

    fn with_pull(
        &self,
        state: &mut State,
        number: &str,
        then: impl FnOnce(&Self, &mut Pull) -> Response,
    ) -> Response {
        let number: u64 = number.parse().unwrap_or(0);
        let pull = state
            .pulls
            .iter_mut()
            .find(|p| p.repo == self.repo && p.number == number);
        match pull {
            Some(pull) => then(self, pull),
            None => message(404, "Not Found"),
        }
    }

    fn pull_json(&self, pull: &Pull) -> Value {
        let (state, merged, commit) = match &pull.state {
            PullState::Open => ("open", false, None),
            PullState::Closed => ("closed", false, None),
            PullState::Merged { commit, .. } => ("closed", true, Some(commit.clone())),
        };
        let head = match &pull.state {
            PullState::Merged { head, .. } => Some(head.clone()),
            _ => self.head_of(&pull.branch),
        };
        json!({
            "number": pull.number,
            "html_url": format!("{}/{}/pull/{}", self.shared.web, self.repo, pull.number),
            "title": pull.title,
            "body": pull.body,
            "state": state,
            "merged": merged,
            "merge_commit_sha": commit,
            "draft": pull.draft,
            "head": { "ref": pull.branch, "sha": head },
            "base": { "ref": pull.base },
        })
    }

    fn owner(&self) -> &str {
        self.repo.split('/').next().unwrap_or_default()
    }

    fn head_of(&self, branch: &str) -> Option<String> {
        self.shared.head_of(&self.repo, branch)
    }

    fn bare(&self, args: &[&str], vars: &[(&str, &str)]) -> Option<String> {
        self.shared.bare(&self.repo, args, vars)
    }
}

/// `POST /_fake/pulls` with `{repo, count}`, for a process that can't call
/// [`FakeGithub::open_elsewhere`](super::FakeGithub::open_elsewhere).
pub(super) fn open_elsewhere(shared: &Shared, request: &Request) -> Response {
    let body: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
    let (Some(repo), Some(count)) = (body["repo"].as_str(), body["count"].as_u64()) else {
        return Response::empty(400);
    };
    *shared
        .state()
        .opened_elsewhere
        .entry(repo.into())
        .or_default() += count;
    Response::empty(204)
}

/// Merges or closes pull request `number` of `repo` from GitHub's own page,
/// as someone there does without the hub: `action` is `merge`, squashing it
/// under GitHub's default title, or `close`.
pub(super) fn act_elsewhere(shared: &Shared, repo: &str, number: u64, action: &str) -> Response {
    let mut state = shared.state();
    let call = Call {
        shared,
        repo: repo.into(),
        can_push: true,
    };
    let number = number.to_string();
    match action {
        "merge" => call.merge(&mut state, &number, &Value::Null),
        "close" => call.update(&mut state, &number, &json!({ "state": "closed" })),
        _ => message(404, "Not Found"),
    }
}

/// `POST /_fake/pulls/{number}/merge` or `.../close` with `{repo}`, for a
/// process that can't call [`FakeGithub`](super::FakeGithub)'s own methods.
pub(super) fn control(shared: &Shared, request: &Request) -> Response {
    let body: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
    let segments: Vec<&str> = request.path.trim_start_matches('/').split('/').collect();
    let (["_fake", "pulls", number, action], Some(repo)) =
        (segments.as_slice(), body["repo"].as_str())
    else {
        return Response::empty(400);
    };
    let Ok(number) = number.parse() else {
        return Response::empty(400);
    };
    act_elsewhere(shared, repo, number, action)
}

fn message(status: u16, message: &str) -> Response {
    Response::json(status, &json!({ "message": message }))
}

fn invalid(detail: &str) -> Response {
    Response::json(
        422,
        &json!({ "message": "Validation Failed", "errors": [{ "message": detail }] }),
    )
}
