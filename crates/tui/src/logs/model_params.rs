#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModelParams {
    pub(crate) model: String,
    pub(crate) reasoning_effort: Option<String>,
}

pub(crate) fn parse_system_message_for_model_params(text: &str) -> Option<ModelParams> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Known formats we see from executors:
    // - "model: gpt-5.2  reasoning effort: low"
    // - "model: gpt-5.2"
    //
    // Keep parsing intentionally simple and robust to extra spaces.
    let mut model: Option<String> = None;
    let mut effort: Option<String> = None;

    let mut rest = trimmed;
    if let Some(after) = rest.strip_prefix("model:") {
        rest = after.trim_start();
        // model token ends at two+ spaces or before "reasoning effort:"
        if let Some(pos) = rest.find("reasoning effort:") {
            let before = rest[..pos].trim();
            if !before.is_empty() {
                model = Some(before.to_string());
            }
            let after_eff = rest[pos..].trim_start();
            if let Some(after_eff) = after_eff.strip_prefix("reasoning effort:") {
                let v = after_eff.trim();
                if !v.is_empty() {
                    effort = Some(v.to_string());
                }
            }
        } else {
            let v = rest.trim();
            if !v.is_empty() {
                model = Some(v.to_string());
            }
        }
    } else if rest.contains("reasoning effort:") {
        // Sometimes we might see an isolated "reasoning effort:" message; parse it but only return
        // if we can also find a model.
        if let Some(pos) = rest.find("reasoning effort:") {
            let after_eff = rest[pos..].trim_start();
            if let Some(after_eff) = after_eff.strip_prefix("reasoning effort:") {
                let v = after_eff.trim();
                if !v.is_empty() {
                    effort = Some(v.to_string());
                }
            }
        }
    }

    let model = model?;
    Some(ModelParams {
        model,
        reasoning_effort: effort,
    })
}

pub(crate) fn is_model_params_system_message(text: &str) -> bool {
    parse_system_message_for_model_params(text).is_some()
}
