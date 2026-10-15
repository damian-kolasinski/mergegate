//! The hub's second listener, on the tailnet (design §4): with `listen` set to
//! `tailnet`, the hub also serves its tailnet address, so workers on other
//! machines reach it, and keeps trying while Tailscale is down.

use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use axum::{Extension, Router};
use mergegate_core::device::{Device, Status, Tailnet as _, TailnetError};
use mergegate_core::settings::Listen;
use tailscale::TailscaleCli;
use tokio::net::TcpListener;
use tokio::sync::Notify;
use tokio::task::JoinHandle;

use crate::server::AppState;

/// Which listener a request came in on, set on each listener's router.
#[derive(Clone)]
pub(crate) enum Via {
    Loopback,
    Tailnet(Arc<Bound>),
}

/// The tailnet listener while it is up.
pub(crate) struct Bound {
    /// What other machines connect to.
    pub url: String,
    /// The `Host` values that address this hub over the tailnet, against DNS
    /// rebinding as on loopback.
    hosts: Vec<String>,
    /// The hub's own machine, whose owner's devices are trusted.
    pub me: Device,
}

impl Bound {
    pub fn addressed_as(&self, host: &str) -> bool {
        self.hosts.iter().any(|h| h.eq_ignore_ascii_case(host))
    }
}

#[derive(Clone)]
pub(crate) enum Listening {
    Off,
    /// Retrying; the problem is the last attempt's.
    Waiting(String),
    Bound(Arc<Bound>),
}

pub(crate) struct Tailnet {
    tailscale: Arc<TailscaleCli>,
    /// The app every listener serves, set once by `server::serve`. It holds
    /// the state that holds it; the cycle lives as long as the process.
    router: OnceLock<Router>,
    /// Held across a change of listener, so two changes can't interleave.
    tasks: tokio::sync::Mutex<Tasks>,
    listening: Mutex<Listening>,
    /// `MG_TAILNET_ADDR`: an address to listen on instead of the tailnet's,
    /// so tests and demos run without a second machine.
    addr: Option<SocketAddr>,
    retry: Duration,
}

#[derive(Default)]
struct Tasks {
    retry: Option<JoinHandle<()>>,
    serve: Option<Arc<Notify>>,
}

impl Tailnet {
    pub fn from_env() -> Self {
        let retry = std::env::var("MG_TAILNET_RETRY_MS")
            .ok()
            .and_then(|ms| ms.parse().ok())
            .unwrap_or(10_000);
        Self {
            tailscale: Arc::new(TailscaleCli::from_env()),
            router: OnceLock::new(),
            tasks: tokio::sync::Mutex::default(),
            listening: Mutex::new(Listening::Off),
            addr: std::env::var("MG_TAILNET_ADDR")
                .ok()
                .and_then(|addr| addr.parse().ok()),
            retry: Duration::from_millis(retry),
        }
    }

    pub fn set_router(&self, router: Router) {
        let _ = self.router.set(router);
    }

    pub fn listening(&self) -> Listening {
        self.listening
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Whether `bound` is still the listener: after `listen local`, a
    /// connection the old listener kept alive gets nothing more.
    pub fn is_current(&self, bound: &Arc<Bound>) -> bool {
        matches!(self.listening(), Listening::Bound(now) if Arc::ptr_eq(&now, bound))
    }

    pub async fn status(&self) -> Result<Status, TailnetError> {
        let tailscale = Arc::clone(&self.tailscale);
        blocking(move || tailscale.status()).await
    }

    pub async fn whois(&self, ip: IpAddr) -> Result<Device, TailnetError> {
        let tailscale = Arc::clone(&self.tailscale);
        blocking(move || tailscale.whois(ip)).await
    }

    pub fn tailscale(&self) -> Arc<TailscaleCli> {
        Arc::clone(&self.tailscale)
    }

    fn set(&self, listening: Listening) {
        *self.listening.lock().unwrap_or_else(|e| e.into_inner()) = listening;
    }
}

/// Listens on the tailnet or stops, as `listen` says. The first attempt runs
/// before this returns, so `mg hub set listen tailnet` can print the URL.
pub(crate) async fn apply(state: &Arc<AppState>, listen: Listen) {
    let tailnet = &state.tailnet;
    let mut tasks = tailnet.tasks.lock().await;
    match listen {
        Listen::Local => {
            if let Some(retry) = tasks.retry.take() {
                retry.abort();
            }
            if let Some(serve) = tasks.serve.take() {
                serve.notify_one();
                tracing::info!("hub stopped listening on the tailnet");
            }
            tailnet.set(Listening::Off);
        }
        Listen::Tailnet => {
            if !matches!(tailnet.listening(), Listening::Off) {
                return;
            }
            if let Err(problem) = bind(state, &mut tasks).await {
                tracing::warn!("not listening on the tailnet yet: {problem}");
                tailnet.set(Listening::Waiting(problem));
                tasks.retry = Some(tokio::spawn(retry(Arc::clone(state))));
            }
        }
    }
}

async fn retry(state: Arc<AppState>) {
    let tailnet = &state.tailnet;
    loop {
        tokio::time::sleep(tailnet.retry).await;
        let mut tasks = tailnet.tasks.lock().await;
        if !matches!(tailnet.listening(), Listening::Waiting(_)) {
            return;
        }
        match bind(&state, &mut tasks).await {
            Ok(()) => {
                tasks.retry = None;
                return;
            }
            Err(problem) => tailnet.set(Listening::Waiting(problem)),
        }
    }
}

async fn bind(state: &Arc<AppState>, tasks: &mut Tasks) -> Result<(), String> {
    let tailnet = &state.tailnet;
    let status = tailnet.status().await.map_err(|e| e.to_string())?;
    let me = status.me;
    let addr = match tailnet.addr {
        Some(addr) => addr,
        None => {
            // Peers reach the hub by its IPv4 address; MagicDNS names
            // resolve to it first.
            let ip = me
                .ips
                .iter()
                .find(|ip| ip.is_ipv4())
                .ok_or_else(|| format!("{} has no tailnet IPv4 address", me.name))?;
            SocketAddr::new(*ip, state.port)
        }
    };
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| format!("can't listen on {addr}: {e}"))?;
    let local = listener
        .local_addr()
        .map_err(|e| format!("can't read the address listened on: {e}"))?;
    let url = match tailnet.addr {
        Some(_) => format!("http://{local}"),
        None => format!("http://{}:{}", me.name, local.port()),
    };
    let mut hosts = vec![
        format!("{}:{}", me.name, local.port()),
        format!("{}:{}", me.dns_name, local.port()),
        local.to_string(),
    ];
    hosts.extend(
        me.ips
            .iter()
            .map(|ip| SocketAddr::new(*ip, local.port()).to_string()),
    );
    let bound = Arc::new(Bound { url, hosts, me });

    let app = tailnet
        .router
        .get()
        .expect("serve sets the router before listening")
        .clone()
        .layer(Extension(Via::Tailnet(Arc::clone(&bound))));
    let stop = Arc::new(Notify::new());
    let stopped = Arc::clone(&stop);
    tokio::spawn(async move {
        let serving = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async move { stopped.notified().await });
        if let Err(e) = serving.await {
            tracing::error!("serving the tailnet failed: {e}");
        }
    });
    tracing::info!("hub listening on the tailnet at {}", bound.url);
    tasks.serve = Some(stop);
    tailnet.set(Listening::Bound(bound));
    Ok(())
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, TailnetError> + Send + 'static,
) -> Result<T, TailnetError> {
    tokio::task::spawn_blocking(f)
        .await
        .unwrap_or_else(|e| std::panic::resume_unwind(e.into_panic()))
}
