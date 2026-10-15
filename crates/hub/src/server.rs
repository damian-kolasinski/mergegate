use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::middleware;
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::Notify;

use backend_github::GithubClient;
use git::HubGit;
use highlight::Languages;
use lang_release::Releases;
use mergegate_core::highlight::Pack;
use rules_toml::RulesToml;
use store_sqlite::Store;

use crate::{
    Home, HubError, HubInfo, abandon, attachments, comments, conversation, diff, doctor,
    draft_replies, event_log, events, external, feedback, git_http, guard, highlights, io, items,
    languages, lifecycle, merge, priority, projects, replies, reply, resume, reviews, rules,
    settings, ui, viewed, wake,
};

pub(crate) struct AppState {
    pub health: api::hub::Health,
    pub port: u16,
    pub shutdown: Notify,
    pub events: events::Events,
    pub wakes: wake::Wakes,
    pub wake_logs: PathBuf,
    /// Held across a pull of pull request comments, from asking GitHub to
    /// storing what it said, so a slower poll can't store older bodies over
    /// what a `mg sync` just brought in.
    pub pulling: tokio::sync::Mutex<()>,
    pub languages: Arc<Languages>,
    /// The packs mg can name for a file no installed pack highlights.
    pub known_packs: Vec<Pack>,
    /// The packs released with this mg, which it downloads by name.
    pub releases: Arc<Releases>,
    /// Wakes the background highlight pass; a wake while it runs makes it
    /// run once more.
    pub highlight: Notify,
    /// Asks the attachment pruner to look now.
    pub prune: Notify,
    pub rules: rules::RulesFiles,
    /// Held for a whole request, which also keeps two requests from syncing
    /// the same staging repo at once.
    store: Mutex<Store>,
    git: HubGit,
    github: GithubClient,
}

impl AppState {
    /// Runs `f` off the async workers: SQLite and git block.
    pub(crate) async fn blocking<T: Send + 'static>(
        self: &Arc<Self>,
        f: impl FnOnce(&mut Store, &HubGit) -> T + Send + 'static,
    ) -> T {
        self.blocking_github(|store, git, _| f(store, git)).await
    }

    /// [`Self::blocking`], with GitHub too. Its calls go over the network
    /// while the store is held, so other requests wait on GitHub.
    pub(crate) async fn blocking_github<T: Send + 'static>(
        self: &Arc<Self>,
        f: impl FnOnce(&mut Store, &HubGit, &GithubClient) -> T + Send + 'static,
    ) -> T {
        let state = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            // A panic mid-request leaves nothing half-applied: every write is
            // a single statement.
            let mut store = state.store.lock().unwrap_or_else(|e| e.into_inner());
            f(&mut store, &state.git, &state.github)
        })
        .await
        .unwrap_or_else(|e| std::panic::resume_unwind(e.into_panic()))
    }

    /// Calls GitHub off the async workers, with the store unlocked.
    pub(crate) async fn github<T: Send + 'static>(
        self: &Arc<Self>,
        f: impl FnOnce(&GithubClient) -> T + Send + 'static,
    ) -> T {
        let state = Arc::clone(self);
        tokio::task::spawn_blocking(move || f(&state.github))
            .await
            .unwrap_or_else(|e| std::panic::resume_unwind(e.into_panic()))
    }

    /// Runs `f` off the async workers without the store, for slow work that
    /// only reads git objects, which a concurrent sync never rewrites.
    pub(crate) async fn blocking_git<T: Send + 'static>(
        self: &Arc<Self>,
        f: impl FnOnce(&HubGit) -> T + Send + 'static,
    ) -> T {
        let state = Arc::clone(self);
        tokio::task::spawn_blocking(move || f(&state.git))
            .await
            .unwrap_or_else(|e| std::panic::resume_unwind(e.into_panic()))
    }
}

pub(crate) async fn serve(
    home: &Home,
    port: u16,
    schema_version: u32,
    store: Store,
) -> Result<(), HubError> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .await
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::AddrInUse => HubError::PortInUse(port),
            _ => io("binding the hub's port")(e),
        })?;
    let addr = listener
        .local_addr()
        .map_err(io("reading the bound address"))?;
    let pid = std::process::id();
    let (languages, warnings) =
        Languages::open(&home.languages()).map_err(io("opening the language packs"))?;
    for warning in warnings {
        tracing::warn!("{warning}");
    }
    let releases = Releases::from_env().unwrap_or_else(|e| {
        tracing::warn!("{e}; language packs install only from files");
        Releases::none()
    });
    let state = Arc::new(AppState {
        health: api::hub::Health {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            api_version: api::API_VERSION,
            schema_version,
            pid,
            home: home.root().display().to_string(),
        },
        port: addr.port(),
        shutdown: Notify::new(),
        events: events::Events::new(),
        wakes: wake::Wakes::default(),
        wake_logs: home.wake_logs(),
        pulling: tokio::sync::Mutex::new(()),
        languages: Arc::new(languages),
        known_packs: highlight::known(),
        releases: Arc::new(releases),
        highlight: Notify::new(),
        prune: Notify::new(),
        rules: rules::RulesFiles::new(RulesToml::new(home.projects())),
        store: Mutex::new(store),
        git: HubGit::new(home.projects(), home.git_home()),
        github: GithubClient::from_env(),
    });
    let app = Router::new()
        .merge(lifecycle::routes())
        .merge(projects::routes())
        .merge(rules::routes())
        .merge(items::routes())
        .merge(abandon::routes())
        .merge(priority::routes())
        .merge(merge::routes())
        .merge(diff::routes())
        .merge(highlights::routes())
        .merge(languages::routes())
        .merge(conversation::routes())
        .merge(viewed::routes())
        .merge(attachments::routes())
        .merge(settings::routes())
        .merge(reviews::routes())
        .merge(comments::routes())
        .merge(replies::routes())
        .merge(draft_replies::routes())
        .merge(feedback::routes())
        .merge(event_log::routes())
        .merge(external::routes())
        .merge(resume::routes())
        .merge(git_http::routes())
        .merge(doctor::routes())
        .merge(events::routes())
        .merge(ui::routes())
        .fallback(reply::no_route)
        .method_not_allowed_fallback(reply::wrong_method)
        .layer(middleware::from_fn_with_state(state.clone(), guard::guard))
        .with_state(state.clone());

    let mut sigterm = signal(SignalKind::terminate()).map_err(io("handling SIGTERM"))?;
    home.write_info(&HubInfo { addr, pid })
        .map_err(io("writing hub.json"))?;
    tracing::info!(
        "hub listening on http://{addr}, data in {}",
        home.root().display()
    );

    tokio::spawn(wake::run(state.clone()));
    tokio::spawn(external::run(state.clone()));
    tokio::spawn(highlights::run(state.clone()));
    tokio::spawn(attachments::prune(state.clone()));
    tokio::spawn(rules::run(state.clone()));
    let result = axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            tokio::select! {
                () = state.shutdown.notified() => {}
                _ = tokio::signal::ctrl_c() => {}
                _ = sigterm.recv() => {}
            }
            tracing::info!("hub stopping");
            state.events.stop();
        })
        .await
        .map_err(io("serving HTTP"));
    home.remove_info(pid);
    result
}
