use crate::state::{AppState, JobKey};

pub(crate) fn cancel_job(app: &mut AppState, key: JobKey) {
    if let Some(job) = app.jobs.remove(&key) {
        job.abort();
    }
}

pub(crate) fn replace_job(app: &mut AppState, key: JobKey, job: tokio::task::JoinHandle<()>) {
    cancel_job(app, key);
    app.jobs.insert(key, job);
}

pub(crate) fn replace_blocking_job<F>(app: &mut AppState, key: JobKey, f: F)
where
    F: FnOnce() + Send + 'static,
{
    replace_job(app, key, tokio::task::spawn_blocking(f));
}

pub(crate) fn job_running(app: &AppState, key: JobKey) -> bool {
    app.jobs.get(&key).is_some_and(|h| !h.is_finished())
}

pub(crate) fn reap_finished_jobs(app: &mut AppState) -> bool {
    let before = app.jobs.len();
    app.jobs.retain(|_, h| !h.is_finished());
    app.jobs.len() != before
}
