//! Model changes release both backends; unchanged workflows keep their caches.
use super::*;
use std::sync::PoisonError;

#[derive(Default)]
pub(super) struct Models {
    image: Option<Vec<String>>,
    translation: Option<String>,
    needs_release: bool,
}

impl ComfyUi {
    pub(super) fn release_models(&self, endpoint: &str, ctx: &JobCtx) -> Result<()> {
        let base = endpoint_url(endpoint)?;
        let mut models = self.models.lock().unwrap_or_else(PoisonError::into_inner);
        models.entry(base.to_string()).or_default().needs_release = true;
        free_models(&base, ctx)?;
        models.remove(base.as_str());
        Ok(())
    }

    pub(super) fn image_models(&self, endpoint: &str, loaders: &[(&str, &str, String)], ctx: &JobCtx) -> Result<()> {
        let base = endpoint_url(endpoint)?;
        let names: Vec<_> = loaders.iter().map(|(_, _, name)| name.clone()).collect();
        let mut models = self.models.lock().unwrap_or_else(PoisonError::into_inner);
        let state = models.entry(base.to_string()).or_default();
        if state.needs_release || state.image.as_ref().is_some_and(|old| *old != names) {
            state.needs_release = true;
            free_models(&base, ctx)?;
            *state = Models::default();
        }
        state.image = Some(names);
        Ok(())
    }

    pub(super) fn translation_model(&self, endpoint: &str, name: &str, ctx: &JobCtx) -> Result<()> {
        let base = endpoint_url(endpoint)?;
        let mut models = self.models.lock().unwrap_or_else(PoisonError::into_inner);
        let state = models.entry(base.to_string()).or_default();
        if state.needs_release || state.translation.as_ref().is_some_and(|old| old != name) {
            state.needs_release = true;
            free_models(&base, ctx)?;
            *state = Models::default();
        }
        state.translation = Some(name.to_owned());
        Ok(())
    }
}

fn free_models(base: &Url, ctx: &JobCtx) -> Result<()> {
    ctx.check()?;
    let client = client(Duration::from_secs(3), Duration::from_secs(5))?;
    ctx.progress(0.02, "Unloading ComfyUI models");
    client
        .post(base.join("free").map_err(failure)?)
        .json(&json!({"unload_models":true,"free_memory":true}))
        .send()
        .and_then(Response::error_for_status)
        .map_err(|error| failure(format!("{error:?}")))?;
    // /free only sets worker flags. A queued marker forces a worker iteration
    // before the next real job; flags are handled before that next job executes.
    let job = uuid::Uuid::new_v4().to_string();
    let marker = read_json(
        client
            .post(base.join("prompt").map_err(failure)?)
            .json(&json!({"prompt_id":job,"client_id":job,"prompt":{"990":{"class_type":"PreviewAny","inputs":{"source":job}}}}))
            .send()
            .map_err(failure)?,
    )?;
    if marker.get("prompt_id").and_then(Value::as_str) != Some(job.as_str()) {
        return Err(failure("unexpected unload marker id"));
    }
    let history = base.join(&format!("history/{job}")).map_err(failure)?;
    let started = Instant::now();
    loop {
        ctx.check()?;
        if started.elapsed() > Duration::from_secs(30) {
            return Err(failure("model unload queue timed out"));
        }
        let result = read_json(client.get(history.clone()).send().map_err(failure)?)?;
        if let Some(record) = result.get(&job) {
            if record.pointer("/status/status_str").and_then(Value::as_str) == Some("error") {
                return Err(failure("model unload marker failed"));
            }
            if record.pointer("/status/completed").and_then(Value::as_bool) == Some(true) {
                return Ok(());
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
