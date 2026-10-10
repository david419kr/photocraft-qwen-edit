//! Native ComfyUI transport. Only an explicit edit submits a job; cancellation is job-scoped.

use std::io::Read;
use std::time::{Duration, Instant};

use photocraft_codecs::{ChannelLayout, DecodeOptions, EncodeOptions, Format, Image, Limits, SampleType};
use photocraft_raster::Rgba8Image;
use reqwest::Url;
use reqwest::blocking::{Client, Response, multipart};
use serde_json::{Value, json};

use crate::ai_cmds::{AiBackend, AiRequest, EditMode};
use crate::jobs::JobCtx;
use crate::prefs::ComfyUiModel;
use crate::{EngineError, Result};

const MAX_RESPONSE_BYTES: u64 = 64 << 20;
const MAX_JSON_BYTES: u64 = 1 << 20;
const GENERATION_TIMEOUT: Duration = Duration::from_secs(20 * 60);
#[cfg(test)]
const UNET_MODEL: &str = "qwen_image_2.1_int8_convrot.safetensors";
const VAE_MODEL: &str = "qwen_image_2.1_vae_bf16.safetensors";

#[derive(Default)]
pub struct ComfyUi;

mod translation;

fn failure(message: impl std::fmt::Display) -> EngineError {
    EngineError::Other(format!("ComfyUI: {message}"))
}

pub fn endpoint_url(endpoint: &str) -> Result<Url> {
    let mut url = Url::parse(endpoint).map_err(failure)?;
    let host = url.host_str().unwrap_or_default().trim_matches(['[', ']']);
    let loopback = host.eq_ignore_ascii_case("localhost") || host.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback());
    if !loopback
        || !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(failure("use a local HTTP(S) server, for example http://127.0.0.1:8188"));
    }
    if !url.path().ends_with('/') {
        url.set_path(&format!("{}/", url.path()));
    }
    Ok(url)
}

fn read_response(response: Response, limit: u64) -> Result<Vec<u8>> {
    let response = response.error_for_status().map_err(failure)?;
    if response.content_length().is_some_and(|n| n > limit) {
        return Err(failure("response is too large"));
    }
    let mut data = Vec::new();
    response.take(limit + 1).read_to_end(&mut data).map_err(failure)?;
    if data.len() as u64 > limit {
        return Err(failure("response is too large"));
    }
    Ok(data)
}

fn read_json(response: Response) -> Result<Value> {
    serde_json::from_slice(&read_response(response, MAX_JSON_BYTES)?).map_err(failure)
}

fn client(connect: Duration, timeout: Duration) -> Result<Client> {
    Client::builder()
        .http1_only()
        .pool_max_idle_per_host(0)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(connect)
        .timeout(timeout)
        .build()
        .map_err(failure)
}

fn model_inputs(client: &Client, base: &Url, ctx: &JobCtx, model: ComfyUiModel) -> Result<Vec<(&'static str, &'static str, String)>> {
    let mut models = Vec::with_capacity(3);
    for (id, node, input, filename) in [
        ("37", model.loader(), "unet_name", model.filename()),
        ("38", "CLIPLoader", "clip_name", model.clip_filename()),
        ("39", "VAELoader", "vae_name", VAE_MODEL),
    ] {
        ctx.check()?;
        let url = base.join(&format!("object_info/{node}")).map_err(failure)?;
        let info = read_json(client.get(url).send().map_err(failure)?)?;
        models.push((id, input, model_name(&info, node, input, filename)?));
    }
    Ok(models)
}

fn model_name(info: &Value, node: &str, input: &str, filename: &str) -> Result<String> {
    let names = info
        .get(node)
        .and_then(|v| v.pointer(&format!("/input/required/{input}/0")))
        .and_then(Value::as_array)
        .ok_or_else(|| failure(format!("model list unavailable for {node}.{input}")))?;
    let mut matches: Vec<&str> =
        names.iter().filter_map(Value::as_str).filter(|name| name.rsplit(['/', '\\']).next().is_some_and(|base| base.eq_ignore_ascii_case(filename))).collect();
    // Preserve the existing root-folder choice; otherwise require one unambiguous model.
    if matches.contains(&filename) {
        return Ok(filename.into());
    }
    matches.sort_unstable();
    matches.dedup();
    match matches.as_slice() {
        [name] => Ok((*name).into()),
        [] => Err(failure(format!("model not found: {filename} ({node}.{input}); check ComfyUI's model folders"))),
        _ => Err(failure(format!("multiple models named {filename}: {}; keep one copy or place the intended model in the root folder", matches.join(", ")))),
    }
}

/// Built from the user's 03 Edit / 04 MaskEdit graphs, with UI-only widgets removed.
pub fn workflow(request: &AiRequest, filename: &str, job: &str) -> Value {
    let loader = if request.model.is_gguf() {
        json!({"class_type":"UnetLoaderGGUF","inputs":{"unet_name":request.model.filename()}})
    } else {
        json!({"class_type":"UNETLoader","inputs":{"unet_name":request.model.filename(),"weight_dtype":"default"}})
    };
    if request.mode == EditMode::PromptGenerate {
        let mut graph = json!({
            "37":loader,
            "38":{"class_type":"CLIPLoader","inputs":{"clip_name":request.model.clip_filename(),"type":"qwen_image","device":"default"}},
            "39":{"class_type":"VAELoader","inputs":{"vae_name":VAE_MODEL}},
            "6":{"class_type":"CLIPTextEncode","inputs":{"clip":["38",0],"text":request.prompt}},
            "63":{"class_type":"ConditioningZeroOut","inputs":{"conditioning":["6",0]}},
            "62":{"class_type":"EmptyLatentImage","inputs":{"width":request.image.width,"height":request.image.height,"batch_size":1}},
            "3":{"class_type":"KSampler","inputs":{"model":["37",0],"positive":["6",0],"negative":["63",0],"latent_image":["62",0],"seed":request.seed,"steps":request.steps,"cfg":1.0,"sampler_name":"euler","scheduler":"simple","denoise":1.0}},
            "8":{"class_type":"VAEDecode","inputs":{"samples":["3",0],"vae":["39",0]}},
            "56":{"class_type":"SaveImage","inputs":{"images":["8",0],"filename_prefix":format!("PhotoCraft/{job}/result")}}
        });
        turbo_sampler(&mut graph, request);
        return graph;
    }
    let masked = request.mode == EditMode::GenerativeFill;
    let cache = if masked { "82" } else { "79" };
    let mut graph = json!({
        "37":loader,
        "38":{"class_type":"CLIPLoader","inputs":{"clip_name":request.model.clip_filename(),"type":"qwen_image","device":"default"}},
        "39":{"class_type":"VAELoader","inputs":{"vae_name":VAE_MODEL}},
        "78":{"class_type":"LoadImage","inputs":{"image":filename}},
        "77":{"class_type":"ResizeImageMaskNode","inputs":{"input":["78",0],"resize_type":"scale to multiple","resize_type.multiple":32,"scale_method":"bicubic"}},
        "76":{"class_type":"GetImageSize","inputs":{"image":["77",0]}},
        "62":{"class_type":"EmptyLatentImage","inputs":{"width":["76",0],"height":["76",1],"batch_size":1}},
        "64":{"class_type":"TextEncodeQwenImage21","inputs":{"clip":["38",0],"vae":["39",0],"prompt":request.prompt,"negative_prompt":"","resolution":0,"images.image_1":["77",0]}},
        "3":{"class_type":"KSampler","inputs":{"model":[cache,0],"positive":["64",0],"negative":["64",1],"latent_image":["62",0],"seed":request.seed,"steps":request.steps,"cfg":1.0,"sampler_name":"euler","scheduler":"simple","denoise":1.0}},
        "8":{"class_type":"VAEDecode","inputs":{"samples":["3",0],"vae":["39",0]}},
        "56":{"class_type":"SaveImage","inputs":{"images":["8",0],"filename_prefix":format!("PhotoCraft/{job}/result")}}
    });
    if let Some(object) = graph.as_object_mut() {
        object.insert(cache.into(), json!({"class_type":"QwenImage21Cache","inputs":{"model":["37",0],"device":"auto","dtype":if request.model.is_gguf() { "default" } else { "int8" }}}));
        if masked && request.removal {
            // Put the annotation in the primary reference; a secondary binary silhouette
            // can target the wrong object, and a secondary colour guide can be copied.
            object.insert("79".into(), json!({"class_type":"EmptyImage","inputs":{"width":["76",0],"height":["76",1],"batch_size":1,"color":65280}}));
            object.insert("80".into(), json!({"class_type":"ImageCompositeMasked","inputs":{"destination":["77",0],"source":["79",0],"x":0,"y":0,"resize_source":false,"mask":["78",1]}}));
            if let Some(inputs) = object.get_mut("64").and_then(|n| n.get_mut("inputs")).and_then(Value::as_object_mut) {
                inputs.insert("images.image_1".into(), json!(["80", 0]));
                inputs.remove("images.image_2");
            }
        } else if masked {
            object.insert("79".into(), json!({"class_type":"MaskToImage","inputs":{"mask":["78",1]}}));
            object.insert("80".into(), json!({"class_type":"ResizeImageMaskNode","inputs":{"input":["79",0],"resize_type":"match size","resize_type.match":["77",0],"resize_type.crop":"center","scale_method":"bicubic"}}));
            if let Some(inputs) = object.get_mut("64").and_then(|n| n.get_mut("inputs")).and_then(Value::as_object_mut) {
                inputs.insert("images.image_2".into(), json!(["80", 0]));
            }
        }
    }
    turbo_sampler(&mut graph, request);
    graph
}

/// Resample the checkpoint's published eight-step curve for explicit user overrides.
fn turbo_sigmas(steps: u32) -> String {
    const SIGMAS: [f64; 9] = [1.0, 0.978453, 0.95418, 0.926626, 0.89508, 0.845148, 0.704534, 0.414568, 0.0];
    let steps = steps.clamp(1, crate::ai_cmds::MAX_STEPS);
    (0..=steps)
        .map(|i| {
            let position = f64::from(i) * 8.0 / f64::from(steps);
            let index = position.floor() as usize;
            let start = SIGMAS.get(index).copied().unwrap_or(0.0);
            let end = SIGMAS.get(index.saturating_add(1)).copied().unwrap_or(start);
            format!("{:.6}", start + (end - start) * position.fract())
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn turbo_sampler(graph: &mut Value, request: &AiRequest) {
    if !request.model.is_turbo() {
        return;
    }
    let Some(inputs) = graph.get("3").and_then(|node| node.get("inputs")).and_then(Value::as_object).cloned() else { return };
    let Some(nodes) = graph.as_object_mut() else { return };
    nodes.insert("90".into(), json!({"class_type":"KSamplerSelect","inputs":{"sampler_name":"euler"}}));
    nodes.insert("91".into(), json!({"class_type":"ManualSigmas","inputs":{"sigmas":turbo_sigmas(request.steps)}}));
    nodes.insert(
        "3".into(),
        json!({"class_type":"SamplerCustom","inputs":{
            "model":inputs.get("model"),"positive":inputs.get("positive"),"negative":inputs.get("negative"),"latent_image":inputs.get("latent_image"),
            "add_noise":true,"noise_seed":request.seed,"cfg":1.0,"sampler":["90",0],"sigmas":["91",0]
        }}),
    );
}

impl AiBackend for ComfyUi {
    fn available_translation_models(&self, endpoint: &str) -> Result<Vec<crate::prefs::ComfyUiTranslationModel>> {
        translation::models(endpoint)
    }

    fn translate(&self, endpoint: &str, model: crate::prefs::ComfyUiTranslationModel, prompt: &str, ctx: &JobCtx) -> Result<String> {
        translation::translate(endpoint, model, prompt, ctx)
    }

    fn check_available(&self, endpoint: &str) -> Result<()> {
        self.available_models(endpoint).map(|_| ())
    }

    fn available_models(&self, endpoint: &str) -> Result<Vec<ComfyUiModel>> {
        let base = endpoint_url(endpoint)?;
        let client = client(Duration::from_secs(1), Duration::from_secs(2))?;
        let info = read_json(client.get(base.join("object_info/UNETLoader").map_err(failure)?).send().map_err(failure)?)?;
        let gguf_info = client.get(base.join("object_info/UnetLoaderGGUF").map_err(failure)?).send().ok().and_then(|response| read_json(response).ok());
        let mut models: Vec<_> = ComfyUiModel::ALL
            .into_iter()
            .filter(|model| {
                let source = if model.is_gguf() { gguf_info.as_ref() } else { Some(&info) };
                source.is_some_and(|source| model_name(source, model.loader(), "unet_name", model.filename()).is_ok())
            })
            .collect();
        if models.is_empty() {
            return Err(failure("no supported Qwen Image 2.1 checkpoint found"));
        }
        let clip_info = read_json(client.get(base.join("object_info/CLIPLoader").map_err(failure)?).send().map_err(failure)?)?;
        models.retain(|model| model_name(&clip_info, "CLIPLoader", "clip_name", model.clip_filename()).is_ok());
        if models.is_empty() {
            return Err(failure("supported text encoder not found"));
        }
        let vae_info = read_json(client.get(base.join("object_info/VAELoader").map_err(failure)?).send().map_err(failure)?)?;
        model_name(&vae_info, "VAELoader", "vae_name", VAE_MODEL)?;
        for node in ["TextEncodeQwenImage21", "QwenImage21Cache"] {
            let info = read_json(client.get(base.join(&format!("object_info/{node}")).map_err(failure)?).send().map_err(failure)?)?;
            if !info.get(node).is_some_and(Value::is_object) {
                return Err(failure(format!("required node not found: {node}")));
            }
        }
        if models.iter().any(|model| model.is_turbo()) {
            for node in ["SamplerCustom", "ManualSigmas", "KSamplerSelect"] {
                let info = read_json(client.get(base.join(&format!("object_info/{node}")).map_err(failure)?).send().map_err(failure)?)?;
                if !info.get(node).is_some_and(Value::is_object) {
                    models.retain(|model| !model.is_turbo());
                    break;
                }
            }
        }
        if models.is_empty() {
            return Err(failure("Turbo sampling nodes are unavailable; update ComfyUI"));
        }
        Ok(models)
    }

    fn generate(&self, request: &AiRequest, ctx: &JobCtx) -> Result<Rgba8Image> {
        let base = endpoint_url(&request.endpoint)?;
        let route = |path: &str| base.join(path).map_err(failure);
        let client = client(Duration::from_secs(3), Duration::from_secs(5))?;
        ctx.check()?;
        let models = model_inputs(&client, &base, ctx, request.model)?;
        ctx.check()?;
        let job = uuid::Uuid::new_v4().to_string();
        let filename = if request.mode == EditMode::PromptGenerate {
            String::new()
        } else {
            let image = Image::from_raw(request.image.width, request.image.height, ChannelLayout::Rgba, SampleType::U8, request.image.pixels.clone())
                .map_err(failure)?;
            let png = photocraft_codecs::encode(&image, Format::Png, &EncodeOptions::default()).map_err(failure)?;
            let part = multipart::Part::bytes(png).file_name("source.png").mime_str("image/png").map_err(failure)?;
            let form =
                multipart::Form::new().part("image", part).text("type", "input").text("subfolder", format!("PhotoCraft/{job}")).text("overwrite", "false");
            ctx.progress(0.1, "Uploading image to ComfyUI");
            let upload =
                read_json(client.post(route("upload/image")?).timeout(Duration::from_secs(30)).multipart(form).send().map_err(|e| failure(format!("{e:?}")))?)?;
            let name = upload.get("name").and_then(Value::as_str).filter(|s| !s.is_empty()).ok_or_else(|| failure("upload response has no image name"))?;
            let folder = upload.get("subfolder").and_then(Value::as_str).unwrap_or_default();
            if folder.is_empty() { name.to_owned() } else { format!("{folder}/{name}") }
        };
        ctx.check()?;
        ctx.progress(0.2, "Waiting for ComfyUI generation");
        let mut prompt = workflow(request, &filename, &job);
        for (id, input, name) in models {
            let value = prompt.pointer_mut(&format!("/{id}/inputs/{input}")).ok_or_else(|| failure("workflow model input missing"))?;
            *value = Value::String(name);
        }
        // A client-selected UUID lets us cancel our job even when the POST reply is lost.
        // POST is never blindly retried: repeating the UUID does not guarantee deduplication.
        let outcome = (|| {
            let response = read_json(
                client
                    .post(route("prompt")?)
                    .timeout(Duration::from_secs(30))
                    .json(&json!({"prompt":prompt,"client_id":job,"prompt_id":job}))
                    .send()
                    .map_err(failure)?,
            )?;
            if response.get("prompt_id").and_then(Value::as_str) != Some(job.as_str()) {
                return Err(failure("unexpected prompt id"));
            }
            let started = Instant::now();
            let history_url = route(&format!("history/{job}"))?;
            let mut read_failures = 0;
            loop {
                ctx.check()?;
                if started.elapsed() > GENERATION_TIMEOUT {
                    return Err(failure("generation timed out"));
                }
                let history = match client.get(history_url.clone()).send().and_then(Response::error_for_status) {
                    Ok(response) => read_json(response)?,
                    Err(_) => {
                        read_failures += 1;
                        if read_failures >= 6 {
                            return Err(failure("connection lost while waiting for generation"));
                        }
                        // A read can be safely retried; cancellation is checked again next iteration.
                        std::thread::sleep(Duration::from_millis(500));
                        continue;
                    }
                };
                read_failures = 0;
                if let Some(record) = history.get(&job) {
                    if record.pointer("/status/status_str").and_then(Value::as_str) == Some("error") {
                        return Err(failure("generation failed; check the ComfyUI job log"));
                    }
                    if let Some(image) = record.pointer("/outputs/56/images/0") {
                        let filename = image.get("filename").and_then(Value::as_str).ok_or_else(|| failure("result has no filename"))?;
                        let subfolder = image.get("subfolder").and_then(Value::as_str).unwrap_or_default();
                        let kind = image.get("type").and_then(Value::as_str).unwrap_or("output");
                        ctx.check()?;
                        ctx.progress(0.9, "Downloading generated image");
                        let response = client
                            .get(route("view")?)
                            .query(&[("filename", filename), ("subfolder", subfolder), ("type", kind)])
                            .timeout(Duration::from_secs(30))
                            .send()
                            .map_err(failure)?;
                        let bytes = read_response(response, MAX_RESPONSE_BYTES)?;
                        return decode_result(&bytes, request.image.width, request.image.height);
                    }
                    if record.pointer("/status/completed").and_then(Value::as_bool) == Some(true) {
                        return Err(failure("job completed without a SaveImage result"));
                    }
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        })();
        if outcome.is_err() || ctx.cancelled() {
            // Never use global /interrupt or clear someone else's queue.
            let _ = client.post(route(&format!("api/jobs/{job}/cancel"))?).send();
        }
        ctx.check()?;
        outcome
    }
}

fn decode_result(bytes: &[u8], width: u32, height: u32) -> Result<Rgba8Image> {
    let limits =
        Limits { max_width: width, max_height: height, max_pixels: u64::from(width) * u64::from(height), max_alloc: u64::from(width) * u64::from(height) * 16 };
    let options = DecodeOptions { limits, ..Default::default() };
    let image = photocraft_codecs::decode_as_with(Format::Png, bytes, &options).map_err(failure)?;
    if image.width() != width || image.height() != height || image.sample_type() != SampleType::U8 || !image.layout().is_rgb() {
        return Err(failure("expected a same-size RGB/RGBA 8-bit PNG"));
    }
    let pixels = if image.layout() == ChannelLayout::Rgba {
        image.into_data()
    } else {
        image.data().chunks_exact(3).flat_map(|p| p.iter().copied().chain(std::iter::once(255))).collect()
    };
    Ok(Rgba8Image { width, height, pixels })
}

#[cfg(test)]
mod tests;
