use mergegate_core::report::{self, Issue, IssueReporter, REPOSITORY, Report, ReportError};
use serde_json::json;
use ureq::http::Method;

use crate::GithubClient;

impl IssueReporter for GithubClient {
    fn find(&self, id: &str) -> Result<Option<Issue>, ReportError> {
        let marker = report::marker(id);
        let mut next = Some(format!("/repos/{REPOSITORY}/issues?state=all&per_page=100"));
        while let Some(path) = next {
            let reply = self
                .api
                .get(&path)
                .map_err(|e| ReportError::NotSent(e.to_string()))?;
            if !reply.ok() {
                return Err(ReportError::NotSent(reply.error().to_string()));
            }
            let issues = reply
                .body
                .as_array()
                .ok_or_else(|| ReportError::NotSent("GitHub returned no issue list".into()))?;
            for issue in issues {
                if issue["pull_request"].is_null()
                    && issue["body"].as_str().is_some_and(|b| b.contains(&marker))
                {
                    return parse(issue).map(Some).map_err(ReportError::NotSent);
                }
            }
            next = reply.next;
        }
        Ok(None)
    }

    fn create(&self, report: &Report<'_>) -> Result<Issue, ReportError> {
        let reply = self
            .api
            .send(
                Method::POST,
                &format!("/repos/{REPOSITORY}/issues"),
                json!({
                    "title": report.title,
                    "body": format!("{}\n\n{}", report.body, report::marker(report.id)),
                }),
            )
            .map_err(|e| match e {
                mergegate_core::github::GithubError::NotLoggedIn(_) => {
                    ReportError::NotSent(e.to_string())
                }
                _ => ReportError::Uncertain(e.to_string()),
            })?;
        if !reply.ok() {
            let message = reply.error().to_string();
            return Err(if (400..500).contains(&reply.status) {
                ReportError::NotSent(message)
            } else {
                ReportError::Uncertain(message)
            });
        }
        parse(&reply.body).map_err(ReportError::Uncertain)
    }
}

fn parse(value: &serde_json::Value) -> Result<Issue, String> {
    let invalid = || "GitHub returned an invalid issue receipt".to_owned();
    Ok(Issue {
        number: value["number"].as_u64().ok_or_else(invalid)?,
        url: value["html_url"].as_str().ok_or_else(invalid)?.to_owned(),
    })
}
