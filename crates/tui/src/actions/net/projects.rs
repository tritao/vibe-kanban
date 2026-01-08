use crate::{
    actions::selection as sel,
    events::StreamStatus,
    net::ops::find_project_for_repo_path_http,
    state::{AppState, ProjectSetupState},
};

pub(super) fn info_loaded(app: &mut AppState, ok: bool, summary: String) -> bool {
    app.info_ok = ok;
    app.info_summary = summary;
    true
}

pub(super) fn project_created(app: &mut AppState, project_id: uuid::Uuid) -> bool {
    app.ui.project_setup = None;
    app.ui.project_setup_dismissed = true;
    sel::select_project(app, Some(project_id));
    true
}

pub(super) fn project_repo_added(app: &mut AppState, project_id: uuid::Uuid) -> bool {
    app.ui.project_setup = None;
    app.ui.project_setup_dismissed = true;
    sel::select_project(app, Some(project_id));
    true
}

pub(super) fn project_match_result(app: &mut AppState, project_id: Option<uuid::Uuid>) -> bool {
    if app.ui.project_setup_dismissed {
        return false;
    }
    if let Some(project_id) = project_id {
        app.ui.project_setup = None;
        sel::select_project(app, Some(project_id));
        return true;
    }

    if app.ui.project_setup.is_none() {
        app.ui.project_setup = Some(ProjectSetupState {
            repo_path: app.ui.launch_repo_path.clone(),
            suggested_project_name: app.ui.launch_suggested_project_name.clone(),
            has_projects: true,
            busy: false,
        });
        return true;
    }
    false
}

pub(super) fn projects_stream_status(app: &mut AppState, status: StreamStatus) -> bool {
    super::stream::apply_status(&mut app.board.projects_status, status)
}

pub(super) fn projects_patch(app: &mut AppState, patch: json_patch::Patch) -> bool {
    fn patch_with_projects_upserts(patch: &json_patch::Patch) -> json_patch::Patch {
        use json_patch::{AddOperation, PatchOperation, ReplaceOperation};
        let mut out: Vec<PatchOperation> = Vec::with_capacity(patch.0.len());
        for op in patch.iter() {
            match op {
                PatchOperation::Replace(ReplaceOperation { path, value }) => {
                    let p = path.to_string();
                    if p.starts_with("/projects/") {
                        out.push(PatchOperation::Add(AddOperation {
                            path: path.clone(),
                            value: value.clone(),
                        }));
                    } else {
                        out.push(PatchOperation::Replace(ReplaceOperation {
                            path: path.clone(),
                            value: value.clone(),
                        }));
                    }
                }
                other => out.push(other.clone()),
            }
        }
        json_patch::Patch(out)
    }

    if let Err(e) = app.board.projects_store.apply_patch(&patch) {
        // Fallback: treat project replaces as upserts to handle cases where we missed an
        // insert patch but still receive a replace for that ID.
        let patched = patch_with_projects_upserts(&patch);
        if let Err(e2) = app.board.projects_store.apply_patch(&patched) {
            app.ui.set_error(format!(
                "failed to apply projects patch: {e} (fallback also failed: {e2})"
            ));
            app.board.projects_status = StreamStatus::Error;
            return true;
        }
    }
    app.board.projects_loaded_once = true;

    sel::reconcile_projects_selection(app);
    let projects_empty =
        crate::store::projects_list::projects_list(app.board.projects_store.as_value()).is_empty();

    if app.ui.launch_dir_explicit
        && !app.ui.launch_match_done
        && app.ui.launch_repo_path.is_some()
        && !app.ui.project_setup_dismissed
    {
        app.ui.launch_match_done = true;
        let repo_path = app.ui.launch_repo_path.clone().unwrap_or_default();
        crate::commands::spawn_net_task(app, move |base_url, net_tx| async move {
            let matched = find_project_for_repo_path_http(&base_url, &repo_path).await;
            match matched {
                Ok(project_id) => {
                    let _ = net_tx
                        .send(crate::events::NetEvent::ProjectMatchResult { project_id })
                        .await;
                }
                Err(e) => {
                    let _ = net_tx
                        .send(crate::events::NetOpError::new("project match", e).into_event())
                        .await;
                    let _ = net_tx
                        .send(crate::events::NetEvent::ProjectMatchResult { project_id: None })
                        .await;
                }
            }
        });
    }

    if app.board.projects_loaded_once
        && projects_empty
        && app.ui.project_setup.is_none()
        && !app.ui.project_setup_dismissed
    {
        app.ui.project_setup = Some(ProjectSetupState {
            repo_path: app.ui.launch_repo_path.clone(),
            suggested_project_name: app.ui.launch_suggested_project_name.clone(),
            has_projects: false,
            busy: false,
        });
        return true;
    }
    true
}
