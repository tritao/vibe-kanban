use crate::slash::composer_completion_items;

#[test]
fn model_autocomplete_offers_effort_directly() {
    let mut app = crate::test_support::mk_app();
    app.ui.composer_active = true;
    app.ui.composer.buffer = "/model ".to_string();
    app.ui.selected_executor_profile = Some(crate::state::ExecutorProfileSelection {
        executor: "CODEX".to_string(),
        variant: Some("DEFAULT".to_string()),
    });
    app.ui.executor_profiles =
        crate::store::executor_profiles::ExecutorProfilesOwned::new(serde_json::json!({
            "CODEX": {
                "DEFAULT": {
                    "CODEX": {
                        "model": "gpt-5.2",
                        "model_reasoning_effort": "high"
                    }
                }
            }
        }));

    let items = composer_completion_items(&app);
    assert!(
        items.iter().any(|i| i.insert.starts_with("--effort ")),
        "expected direct effort completions"
    );
    assert!(
        items.iter().any(|i| i.insert == "--effort high "),
        "expected a specific effort option"
    );
}
