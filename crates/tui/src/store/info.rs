use serde_json::Value;

use crate::state::ExecutorProfileSelection;

pub(crate) struct InfoExecutorProfiles {
    pub(crate) available: Vec<String>,
    pub(crate) selected: Option<ExecutorProfileSelection>,
    pub(crate) profiles_executors: Value,
}

pub(crate) fn extract_executor_profiles(info: &Value) -> InfoExecutorProfiles {
    let available = info
        .get("executors")
        .and_then(|v| v.as_object())
        .map(|o| {
            let mut keys: Vec<String> = o.keys().cloned().collect();
            keys.sort();
            keys
        })
        .unwrap_or_default();

    let profiles_executors = info
        .get("executors")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));

    let selected = info
        .get("config")
        .and_then(|c| c.get("executor_profile"))
        .and_then(|p| serde_json::from_value::<ExecutorProfileSelection>(p.clone()).ok());

    InfoExecutorProfiles {
        available,
        selected,
        profiles_executors,
    }
}

pub(crate) fn summarize_info(info: &Value) -> Option<String> {
    let env = info.get("environment")?;
    let os_type = env.get("os_type")?.as_str().unwrap_or("unknown");
    let os_arch = env
        .get("os_architecture")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let login_status = info.get("login_status")?;

    Some(format!(
        "env: {os_type} ({os_arch}) | login_status: {}",
        login_status_summary(login_status)
    ))
}

pub(crate) fn login_status_summary(v: &Value) -> String {
    if v.get("LoggedOut").is_some() {
        return "logged_out".to_string();
    }
    if let Some(obj) = v.get("LoggedIn").and_then(|x| x.as_object()) {
        if let Some(user) = obj.get("user_id").and_then(|x| x.as_str()) {
            return format!("logged_in({})", &user[..user.len().min(8)]);
        }
        return "logged_in".to_string();
    }
    "unknown".to_string()
}
