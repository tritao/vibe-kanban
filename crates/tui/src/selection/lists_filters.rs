use uuid::Uuid;

use crate::{
    state::{AppState, ExecRow, ExecStatus, RunReason},
    store::projects_list,
};

fn contains_ci(haystack: &str, needle: &str) -> bool {
    let needle = needle.trim();
    if needle.is_empty() {
        return true;
    }
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

pub(crate) fn filtered_projects(app: &AppState) -> Vec<projects_list::ProjectRow> {
    let mut list = projects_list::projects_list(app.board.projects_store.as_value());
    let q = app.board.project_filter.trim();
    if !q.is_empty() {
        list.retain(|p| contains_ci(&p.name, q));
    }
    list
}

pub(crate) fn active_exec_id(execs: &[ExecRow]) -> Option<Uuid> {
    let mut filtered: Vec<&ExecRow> = execs.iter().filter(|e| !e.dropped).collect();
    if filtered.is_empty() {
        return None;
    }
    // exec_list is sorted oldest -> newest, keep that invariant for selection.
    filtered.sort_by(|a, b| a.created_at.cmp(&b.created_at));

    let running: Vec<&ExecRow> = filtered
        .iter()
        .copied()
        .filter(|e| e.status == Some(ExecStatus::Running))
        .collect();
    if !running.is_empty() {
        if let Some(non_dev) = running
            .iter()
            .copied()
            .find(|e| e.run_reason != Some(RunReason::DevServer))
        {
            return Some(non_dev.id);
        }
        return Some(running[running.len() - 1].id);
    }

    if let Some(agent) = filtered
        .iter()
        .rev()
        .copied()
        .find(|e| e.run_reason == Some(RunReason::CodingAgent))
    {
        return Some(agent.id);
    }

    Some(filtered[filtered.len() - 1].id)
}
