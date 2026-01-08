use std::future::Future;

use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    events::NetEvent,
    jobs::latest::{LatestByKey, LatestGen},
    state::{AppState, JobKey},
};

pub(crate) fn run_latest<GetGen, F, Fut>(
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
    crate::commands::run_latest_job(app, key, get_gen, job)
}

pub(crate) fn run_latest_for_repo<GetMap, F, Fut>(
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
    crate::commands::run_latest_job_for_repo(app, key, repo_id, get_map, job)
}
