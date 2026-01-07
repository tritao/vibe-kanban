use std::future::Future;

use tokio::sync::mpsc;

use crate::{
    events::NetEvent,
    jobs::replace_job,
    state::{AppState, JobKey},
};

pub(crate) fn run_net_job<F, Fut>(app: &mut AppState, key: JobKey, f: F)
where
    F: FnOnce(String, mpsc::Sender<NetEvent>) -> Fut,
    Fut: Future<Output = ()> + Send + 'static,
{
    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    replace_job(app, key, tokio::spawn(f(base_url, net_tx)));
}

pub(crate) fn spawn_net_task<F, Fut>(app: &AppState, f: F) -> tokio::task::JoinHandle<()>
where
    F: FnOnce(String, mpsc::Sender<NetEvent>) -> Fut,
    Fut: Future<Output = ()> + Send + 'static,
{
    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    tokio::spawn(f(base_url, net_tx))
}
