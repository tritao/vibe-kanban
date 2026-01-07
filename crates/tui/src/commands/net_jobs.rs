use std::{collections::HashMap, future::Future};

use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    events::NetEvent,
    state::{AppState, JobKey},
};

pub(crate) fn run_latest_job<GetGen, F, Fut>(
    app: &mut AppState,
    key: JobKey,
    get_gen: GetGen,
    job: F,
) -> u64
where
    GetGen: FnOnce(&mut AppState) -> &mut u64,
    F: FnOnce(String, mpsc::Sender<NetEvent>, u64) -> Fut,
    Fut: Future<Output = ()> + Send + 'static,
{
    super::job_runner::run_net_job_latest(
        app,
        key,
        move |app| crate::jobs::latest::next_generation(get_gen(app)),
        job,
    )
}

pub(crate) fn run_latest_job_for_repo<GetMap, F, Fut>(
    app: &mut AppState,
    key: JobKey,
    repo_id: Uuid,
    get_map: GetMap,
    job: F,
) -> u64
where
    GetMap: FnOnce(&mut AppState) -> &mut HashMap<Uuid, u64>,
    F: FnOnce(String, mpsc::Sender<NetEvent>, u64) -> Fut,
    Fut: Future<Output = ()> + Send + 'static,
{
    super::job_runner::run_net_job_latest(
        app,
        key,
        move |app| crate::jobs::latest::next_generation_for(get_map(app), repo_id),
        job,
    )
}
