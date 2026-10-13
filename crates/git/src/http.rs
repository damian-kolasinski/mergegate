//! The hub as a git remote: `git http-backend`, run as a CGI script, serves
//! fetches and pushes to the staging repos.

use std::process::Command;

use mergegate_core::project::ProjectName;

use crate::HubGit;
use crate::command::git;

#[derive(Debug, Clone, Copy, Default)]
pub struct HttpRequest<'a> {
    pub method: &'a str,
    /// The URL path after the project, as in `/info/refs`.
    pub path: &'a str,
    pub query: &'a str,
    pub content_type: Option<&'a str>,
    pub content_length: Option<&'a str>,
    pub content_encoding: Option<&'a str>,
    /// Selects protocol v2.
    pub git_protocol: Option<&'a str>,
}

impl HubGit {
    /// Stdin takes the request body; stdout gives CGI headers, a blank line,
    /// then the response body.
    ///
    /// Pushes are accepted, except to `refs/mergegate`, where the hub pins
    /// revisions; those refs aren't advertised either. `hideRefs` binds only
    /// the smart protocol, so the dumb one, which lists every ref and serves
    /// any object, is off.
    pub fn http_backend(&self, project: &ProjectName, request: &HttpRequest) -> Command {
        let mut cmd = git(&self.home);
        cmd.args([
            "-c",
            "http.receivepack=true",
            "-c",
            "http.getanyfile=false",
            "-c",
            "transfer.hideRefs=refs/mergegate",
            "http-backend",
        ])
        .env("GIT_PROJECT_ROOT", &self.projects)
        .env("GIT_HTTP_EXPORT_ALL", "1")
        .env(
            "PATH_INFO",
            format!("/{project}/staging.git{}", request.path),
        )
        .env("REQUEST_METHOD", request.method)
        .env("QUERY_STRING", request.query);
        let optional = [
            ("CONTENT_TYPE", request.content_type),
            ("CONTENT_LENGTH", request.content_length),
            ("HTTP_CONTENT_ENCODING", request.content_encoding),
            ("HTTP_GIT_PROTOCOL", request.git_protocol),
        ];
        for (name, value) in optional {
            if let Some(value) = value {
                cmd.env(name, value);
            }
        }
        cmd
    }
}
