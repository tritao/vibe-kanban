use std::future::Future;

use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    events::NetEvent,
    jobs::{
        latest::{LatestByKey, LatestGen},
        replace_job,
    },
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

pub(crate) fn run_net_job_latest<Next, F, Fut>(
    app: &mut AppState,
    key: JobKey,
    next_generation: Next,
    job: F,
) -> u64
where
    Next: FnOnce(&mut AppState) -> u64,
    F: FnOnce(String, mpsc::Sender<NetEvent>, u64) -> Fut,
    Fut: Future<Output = ()> + Send + 'static,
{
    let generation = next_generation(app);

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    replace_job(app, key, tokio::spawn(job(base_url, net_tx, generation)));
    generation
}

pub(crate) fn run_latest_job<GetGen, F, Fut>(
    app: &mut AppState,
    key: JobKey,
    get_gen: GetGen,
    job: F,
) -> u64
where
    GetGen: FnOnce(&mut AppState) -> &mut LatestGen,
    F: FnOnce(String, mpsc::Sender<NetEvent>, u64) -> Fut,
    Fut: Future<Output = ()> + Send + 'static,
{
    run_net_job_latest(app, key, move |app| get_gen(app).next(), job)
}

pub(crate) fn run_latest_job_for_repo<GetMap, F, Fut>(
    app: &mut AppState,
    key: JobKey,
    repo_id: Uuid,
    get_map: GetMap,
    job: F,
) -> u64
where
    GetMap: FnOnce(&mut AppState) -> &mut LatestByKey<Uuid>,
    F: FnOnce(String, mpsc::Sender<NetEvent>, u64) -> Fut,
    Fut: Future<Output = ()> + Send + 'static,
{
    run_net_job_latest(app, key, move |app| get_map(app).next_for(repo_id), job)
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
