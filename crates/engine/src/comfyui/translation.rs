use super::*;
use crate::prefs::ComfyUiTranslationModel;

const NODE: &str = "HyMT_Translation_GGUF";

pub(super) fn models(endpoint: &str) -> Result<Vec<ComfyUiTranslationModel>> {
    let base = endpoint_url(endpoint)?;
    let client = client(Duration::from_secs(1), Duration::from_secs(2))?;
    let response = client.get(base.join(&format!("object_info/{NODE}")).map_err(failure)?).send().map_err(failure)?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(Vec::new());
    }
    let info = read_json(response)?;
    let detected: Vec<_> = ComfyUiTranslationModel::ALL.into_iter().filter(|model| model_name(&info, NODE, "model", model.filename()).is_ok()).collect();
    if !detected.is_empty() {
        let preview = read_json(client.get(base.join("object_info/PreviewAny").map_err(failure)?).send().map_err(failure)?)?;
        if !preview.get("PreviewAny").is_some_and(Value::is_object) {
            return Ok(Vec::new());
        }
    }
    Ok(detected)
}

fn workflow(model: &str, prompt: &str) -> Value {
    json!({
        "901":{"class_type":NODE,"inputs":{"model":model,"translate_style":"Default","translate_to":"English","text":prompt,"unload_after_run":false}},
        "902":{"class_type":"PreviewAny","inputs":{"source":["901",0]}}
    })
}

pub(super) fn translate(endpoint: &str, model: ComfyUiTranslationModel, prompt: &str, ctx: &JobCtx) -> Result<String> {
    let base = endpoint_url(endpoint)?;
    let route = |path: &str| base.join(path).map_err(failure);
    let client = client(Duration::from_secs(3), Duration::from_secs(5))?;
    ctx.check()?;
    let info = read_json(client.get(route(&format!("object_info/{NODE}"))?).send().map_err(failure)?)?;
    let model = model_name(&info, NODE, "model", model.filename())?;
    let job = uuid::Uuid::new_v4().to_string();
    let outcome = (|| {
        ctx.check()?;
        let response = read_json(
            client
                .post(route("prompt")?)
                .timeout(Duration::from_secs(30))
                .json(&json!({"prompt":workflow(&model,prompt),"client_id":job,"prompt_id":job}))
                .send()
                .map_err(failure)?,
        )?;
        if response.get("prompt_id").and_then(Value::as_str) != Some(job.as_str()) {
            return Err(failure("unexpected translation prompt id"));
        }
        let started = Instant::now();
        let mut failures = 0;
        loop {
            ctx.check()?;
            if started.elapsed() > GENERATION_TIMEOUT {
                return Err(failure("prompt translation timed out"));
            }
            let history = match client.get(route(&format!("history/{job}"))?).send().and_then(Response::error_for_status) {
                Ok(response) => read_json(response)?,
                Err(_) => {
                    failures += 1;
                    if failures >= 6 {
                        return Err(failure("connection lost during prompt translation"));
                    }
                    std::thread::sleep(Duration::from_millis(500));
                    continue;
                }
            };
            failures = 0;
            if let Some(record) = history.get(&job) {
                if record.pointer("/status/status_str").and_then(Value::as_str) == Some("error") {
                    return Err(failure("prompt translation failed; check the ComfyUI job log"));
                }
                if let Some(text) = record.pointer("/outputs/902/text/0").and_then(Value::as_str) {
                    if text.trim().is_empty() || text.len() > crate::ai_cmds::MAX_PROMPT_BYTES {
                        return Err(failure("translated prompt must contain 1..8192 bytes"));
                    }
                    return Ok(text.to_owned());
                }
                if record.pointer("/status/completed").and_then(Value::as_bool) == Some(true) {
                    return Err(failure("translation job completed without text"));
                }
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    })();
    if outcome.is_err() || ctx.cancelled() {
        let _ = client.post(route(&format!("api/jobs/{job}/cancel"))?).send();
    }
    ctx.check()?;
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn translation_graph_is_local_english_default_style_and_keeps_model_loaded() {
        let graph = workflow(ComfyUiTranslationModel::HyMt18Heretic.filename(), "파란 주전자로 바꿔주세요");
        assert_eq!(graph["901"]["class_type"], NODE);
        assert_eq!(graph["901"]["inputs"]["translate_to"], "English");
        assert_eq!(graph["901"]["inputs"]["translate_style"], "Default");
        assert_eq!(graph["901"]["inputs"]["unload_after_run"], false);
        assert_eq!(graph["902"]["inputs"]["source"], json!(["901", 0]));
    }
}
