use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use axum::middleware;
use axum::{Extension, Router};
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::Notify;
use tracing::Instrument;

use backend_github::GithubClient;
use git::HubGit;
use highlight::Languages;
use lang_release::Releases;
use mergegate_core::highlight::Pack;
use mergegate_core::settings::Listen;
use mergegate_core::store::Shared;
use rules_toml::RulesToml;
use store_sqlite::Store;

use crate::{
    Home, HubError, HubInfo, abandon, attachments, branches, comments, conflicts, conversation,
    demo_zip, devices, diagnostics, diff, doctor, draft_replies, event_log, events, external,
    feedback, git_http, guard, highlights, io, items, judge, languages, lifecycle, merge, names,
    presence, priority, projects, remote_wakes, replies, reply, resume, review_time, reviews,
    rules, settings, tailnet, telemetry, ui, viewed, wake,
};

pub(crate) struct AppState {
    pub health: api::hub::Health,
    pub port: u16,
    pub shutdown: Notify,
    pub events: events::Events,
    pub wakes: wake::Wakes,
    pub presence: presence::Readers,
    pub bots: external::bots::Bots,
    pub wake_logs: PathBuf,
    pub wake_bin: PathBuf,
    /// Held across a pull of pull request comments, from asking GitHub to
    /// storing what it said, so a slower poll can't store older bodies over
    /// what a `mg sync` just brought in.
    pub pulling: tokio::sync::Mutex<()>,
    /// Held across a merge, which lets go of the store while it lands, and
    /// by everything else that moves an item (a submit, an abandon, a close
    /// followed from GitHub) or syncs staging's target, so none moves one or
    /// races its fetch under a landing. Taken before the store, never while
    /// holding it, and only through [`Self::landing`].
    landing: Arc<tokio::sync::Mutex<()>>,
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
    /// Asks the conflict watch to test-merge now, after its current pass.
    pub conflicts: Notify,
    /// Wakes the background judge pass, as [`Self::highlight`] does its own.
    pub judge: Notify,
    pub judge_failures: judge::Failures,
    /// Set once the hub starts stopping, for work off the runtime that
    /// would hold up its exit: a judge call runs for minutes.
    pub stopping: Arc<AtomicBool>,
    pub rules: rules::RulesFiles,
    pub tailnet: tailnet::Tailnet,
    /// Held for a whole request, which also keeps two requests from syncing
    /// the same staging repo at once. A merge holds it a step at a time and
    /// syncs staging between steps, under [`Self::landing`] instead, as is
    /// every other sync of staging's target.
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
        let span = tracing::Span::current();
        tokio::task::spawn_blocking(move || {
            let _span = span.enter();
            // A panic mid-request leaves nothing half-applied: every write is
            // a single statement.
            let mut store = state.store.lock().unwrap_or_else(|e| e.into_inner());
            f(&mut store, &state.git, &state.github)
        })
        .await
        .unwrap_or_else(|e| std::panic::resume_unwind(e.into_panic()))
    }

    /// [`Self::blocking_github`], but `f` takes the store a step at a time,
    /// so requests go on between its steps, as while a merge calls GitHub.
    /// Under [`Self::landing`] throughout.
    pub(crate) async fn stepwise<T: Send + 'static>(
        self: &Arc<Self>,
        f: impl FnOnce(&Steps<'_>, &HubGit, &GithubClient) -> T + Send + 'static,
    ) -> T {
        let landing = self.landing().await;
        let state = Arc::clone(self);
        let span = tracing::Span::current();
        tokio::task::spawn_blocking(move || {
            let _span = span.enter();
            let _landing = landing;
            f(&Steps(&state.store), &state.git, &state.github)
        })
        .await
        .unwrap_or_else(|e| std::panic::resume_unwind(e.into_panic()))
    }

    /// Takes [`Self::landing`]. Owned, so the blocking work it guards holds
    /// it: a client gone mid-request drops the request's future, and a guard
    /// held there would let another landing start under the one still running.
    pub(crate) async fn landing(&self) -> tokio::sync::OwnedMutexGuard<()> {
        Arc::clone(&self.landing).lock_owned().await
    }

    /// Whether something holds [`Self::landing`], as a merge does between
    /// its steps. Asked after reading the store: an item seen in Merging is
    /// then either still landing or, if it finished since, about to say so.
    pub(crate) fn landing_now(&self) -> bool {
        self.landing.try_lock().is_err()
    }

    /// Runs `f` to the end even if the client goes: a merge's events, sent
    /// once it lands, would otherwise go with the request.
    pub(crate) async fn detached<T: Send + 'static>(
        f: impl Future<Output = T> + Send + 'static,
    ) -> T {
        tokio::spawn(f.in_current_span())
            .await
            .unwrap_or_else(|e| std::panic::resume_unwind(e.into_panic()))
    }

    /// Calls GitHub off the async workers, with the store unlocked.
    pub(crate) async fn github<T: Send + 'static>(
        self: &Arc<Self>,
        f: impl FnOnce(&GithubClient) -> T + Send + 'static,
    ) -> T {
        let state = Arc::clone(self);
        let span = tracing::Span::current();
        tokio::task::spawn_blocking(move || span.in_scope(|| f(&state.github)))
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
        let span = tracing::Span::current();
        tokio::task::spawn_blocking(move || span.in_scope(|| f(&state.git)))
            .await
            .unwrap_or_else(|e| std::panic::resume_unwind(e.into_panic()))
    }

    /// [`Self::blocking_git`], with GitHub too, for the target watch: its
    /// fetch writes only the watch's own refs, which nothing else touches.
    pub(crate) async fn unlocked<T: Send + 'static>(
        self: &Arc<Self>,
        f: impl FnOnce(&HubGit, &GithubClient) -> T + Send + 'static,
    ) -> T {
        let state = Arc::clone(self);
        let span = tracing::Span::current();
        tokio::task::spawn_blocking(move || span.in_scope(|| f(&state.git, &state.github)))
            .await
            .unwrap_or_else(|e| std::panic::resume_unwind(e.into_panic()))
    }
}

/// The store as [`AppState::stepwise`] hands it out: locked for each step.
pub(crate) struct Steps<'a>(&'a Mutex<Store>);

impl Shared<Store> for Steps<'_> {
    fn with<T>(&self, step: impl FnOnce(&mut Store) -> T) -> T {
        // As in `blocking_github`: a panicked step left nothing half-applied.
        step(&mut self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

pub(crate) async fn serve(
    home: &Home,
    port: u16,
    version: &str,
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
            version: version.to_owned(),
            api_version: api::API_VERSION,
            schema_version,
            pid,
            home: home.root().display().to_string(),
        },
        port: addr.port(),
        shutdown: Notify::new(),
        events: events::Events::new(),
        wakes: wake::Wakes::default(),
        presence: presence::Readers::default(),
        bots: external::bots::Bots::default(),
        wake_logs: home.wake_logs(),
        wake_bin: home.wake_bin(),
        pulling: tokio::sync::Mutex::new(()),
        landing: Arc::default(),
        languages: Arc::new(languages),
        known_packs: highlight::known(),
        releases: Arc::new(releases),
        highlight: Notify::new(),
        prune: Notify::new(),
        conflicts: Notify::new(),
        judge: Notify::new(),
        judge_failures: judge::Failures::default(),
        stopping: Arc::default(),
        rules: rules::RulesFiles::new(RulesToml::new(home.projects())),
        tailnet: tailnet::Tailnet::from_env(),
        store: Mutex::new(store),
        git: HubGit::new(home.projects(), home.git_home()),
        github: GithubClient::from_env(),
    });
    let app = Router::new()
        .merge(lifecycle::routes())
        .merge(projects::routes())
        .merge(rules::routes())
        .merge(judge::routes())
        .merge(items::routes())
        .merge(abandon::routes())
        .merge(priority::routes())
        .merge(merge::routes())
        .merge(names::routes())
        .merge(diff::routes())
        .merge(highlights::routes())
        .merge(languages::routes())
        .merge(conversation::routes())
        .merge(viewed::routes())
        .merge(review_time::routes())
        .merge(attachments::routes())
        .merge(demo_zip::routes())
        .merge(settings::routes())
        .merge(devices::routes())
        .merge(reviews::routes())
        .merge(comments::routes())
        .merge(replies::routes())
        .merge(draft_replies::routes())
        .merge(feedback::routes())
        .merge(remote_wakes::routes())
        .merge(event_log::routes())
        .merge(external::routes())
        .merge(resume::routes())
        .merge(git_http::routes())
        .merge(doctor::routes())
        .merge(diagnostics::routes())
        .merge(events::routes())
        .merge(ui::routes())
        .fallback(reply::no_route)
        .method_not_allowed_fallback(reply::wrong_method)
        .layer(middleware::from_fn_with_state(state.clone(), guard::guard))
        .layer(middleware::from_fn(telemetry::record_request))
        .with_state(state.clone());
    state.tailnet.set_router(app.clone());
    let app = app.layer(Extension(tailnet::Via::Loopback));
    let stored = state.blocking(|store, _| mergegate_core::settings::settings(store));
    match stored.await {
        Ok(settings) => tailnet::apply(&state, settings.listen).await,
        Err(e) => tracing::error!("reading the hub's settings: {e}"),
    }

    let mut sigterm = signal(SignalKind::terminate()).map_err(io("handling SIGTERM"))?;
    home.write_info(&HubInfo { addr, pid })
        .map_err(io("writing hub.json"))?;
    tracing::info!(
        "hub listening on http://{addr}, data in {}",
        home.root().display()
    );

    branches::sweep(&state).await;
    telemetry::spawn("wake", wake::run(state.clone()));
    telemetry::spawn("external", external::run(state.clone()));
    telemetry::spawn("languages", languages::refresh(state.clone()));
    telemetry::spawn("highlights", highlights::run(state.clone()));
    telemetry::spawn("attachments", attachments::prune(state.clone()));
    telemetry::spawn("rules", rules::run(state.clone()));
    telemetry::spawn("conflicts", conflicts::run(state.clone()));
    telemetry::spawn("judge", judge::run(state.clone()));
    let result = axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            tokio::select! {
                () = state.shutdown.notified() => {}
                _ = tokio::signal::ctrl_c() => {}
                _ = sigterm.recv() => {}
            }
            tracing::info!("hub stopping");
            state.stopping.store(true, Ordering::Relaxed);
            state.events.stop();
            tailnet::apply(&state, Listen::Local).await;
        })
        .await
        .map_err(io("serving HTTP"));
    home.remove_info(pid);
    result
}
