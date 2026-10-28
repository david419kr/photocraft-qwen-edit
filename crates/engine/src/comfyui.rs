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
pub struct ComfyUi {
    models: std::sync::Mutex<std::collections::HashMap<String, residency::Models>>,
}

mod progress;
mod residency;
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

fn model_inputs(client: &Client, base: &Url, ctx: &JobCtx, request: &AiRequest) -> Result<Vec<(&'static str, &'static str, String)>> {
    let model = request.model;
    let mut models = Vec::with_capacity(3);
    for (id, node, input, filename) in [
        ("37", model.loader(), "unet_name", request.custom_model.as_deref().unwrap_or(model.filename())),
        ("38", "CLIPLoader", "clip_name", request.text_encoder.as_str()),
        ("39", "VAELoader", "vae_name", VAE_MODEL),
    ] {
        ctx.check()?;
        let url = base.join(&format!("object_info/{node}")).map_err(failure)?;
        let info = read_json(client.get(url).send().map_err(failure)?)?;
        let name = model_name(&info, node, input, filename)?;
        models.push((id, input, name));
    }
    Ok(models)
}

fn model_name(info: &Value, node: &str, input: &str, filename: &str) -> Result<String> {
    let names = info
        .get(node)
        .and_then(|v| v.pointer(&format!("/input/required/{input}/0")))
        .and_then(Value::as_array)
        .ok_or_else(|| failure(format!("model list unavailable for {node}.{input}")))?;
    let requested = filename.replace('\\', "/");
    let mut matches: Vec<&str> = names
        .iter()
        .filter_map(Value::as_str)
        .filter(|name| {
            name.replace('\\', "/").eq_ignore_ascii_case(&requested) || name.rsplit(['/', '\\']).next().is_some_and(|base| base.eq_ignore_ascii_case(filename))
        })
        .collect();
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

fn detected_encoders(info: &Value, registered: &[String]) -> Vec<String> {
    let mut detected = Vec::new();
    for name in std::iter::once(crate::ai_models::DEFAULT_ENCODER).chain(registered.iter().map(String::as_str)) {
        if let Ok(path) = model_name(info, "CLIPLoader", "clip_name", name)
            && !detected.contains(&path)
        {
            detected.push(path);
        }
    }
    detected
}

/// Built from the user's 03 Edit / 04 MaskEdit graphs, with UI-only widgets removed.
/// For a reference request, use `workflow_with_reference` with its uploaded filename.
pub fn workflow(request: &AiRequest, filename: &str, job: &str) -> Value {
    workflow_with_reference(request, filename, None, job)
}

pub fn workflow_with_reference(request: &AiRequest, filename: &str, reference_filename: Option<&str>, job: &str) -> Value {
    workflow_with_references(request, filename, reference_filename, None, job)
}

pub fn workflow_with_references(request: &AiRequest, filename: &str, reference_filename: Option<&str>, guide_filename: Option<&str>, job: &str) -> Value {
    workflow_with_sampling(request, filename, reference_filename, guide_filename, None, job)
}

/// Uploaded conditioning paths plus the initial-RGBA/noise pair for alpha refinement.
pub fn workflow_with_sampling(
    request: &AiRequest,
    filename: &str,
    reference_filename: Option<&str>,
    guide_filename: Option<&str>,
    sampling_files: Option<(&str, &str)>,
    job: &str,
) -> Value {
    let loader =
        json!({"class_type":"UNETLoader","inputs":{"unet_name":request.custom_model.as_deref().unwrap_or(request.model.filename()),"weight_dtype":"default"}});
    if request.mode == EditMode::PromptGenerate {
        let mut graph = json!({
            "37":loader,
            "38":{"class_type":"CLIPLoader","inputs":{"clip_name":request.text_encoder,"type":"qwen_image","device":"default"}},
            "39":{"class_type":"VAELoader","inputs":{"vae_name":VAE_MODEL}},
            "6":{"class_type":"CLIPTextEncode","inputs":{"clip":["38",0],"text":request.prompt}},
            "63":{"class_type":"ConditioningZeroOut","inputs":{"conditioning":["6",0]}},
            "62":{"class_type":"EmptyLatentImage","inputs":{"width":request.image.width,"height":request.image.height,"batch_size":1}},
            "3":{"class_type":"KSampler","inputs":{"model":["37",0],"positive":["6",0],"negative":["63",0],"latent_image":["62",0],"seed":request.seed,"steps":request.steps,"cfg":request.cfg,"sampler_name":"euler","scheduler":"simple","denoise":1.0}},
            "8":{"class_type":"VAEDecode","inputs":{"samples":["3",0],"vae":["39",0]}},
            "56":{"class_type":"SaveImage","inputs":{"images":["8",0],"filename_prefix":format!("PhotoCraft/{job}/result")}}
        });
        if request.cfg > 1.0
            && let Some(nodes) = graph.as_object_mut()
        {
            nodes.insert("63".into(), json!({"class_type":"CLIPTextEncode","inputs":{"clip":["38",0],"text":request.negative_prompt}}));
        }
        turbo_sampler(&mut graph, request);
        return graph;
    }
    let masked = request.mode == EditMode::GenerativeFill;
    let cache = if masked { "82" } else { "79" };
    let mut graph = json!({
        "37":loader,
        "38":{"class_type":"CLIPLoader","inputs":{"clip_name":request.text_encoder,"type":"qwen_image","device":"default"}},
        "39":{"class_type":"VAELoader","inputs":{"vae_name":VAE_MODEL}},
        "78":{"class_type":"LoadImage","inputs":{"image":filename}},
        "77":{"class_type":"ResizeImageMaskNode","inputs":{"input":["78",0],"resize_type":"scale to multiple","resize_type.multiple":32,"scale_method":"bicubic"}},
        "76":{"class_type":"GetImageSize","inputs":{"image":["77",0]}},
        "62":{"class_type":"EmptyLatentImage","inputs":{"width":["76",0],"height":["76",1],"batch_size":1}},
        "64":{"class_type":"TextEncodeQwenImage21","inputs":{"clip":["38",0],"vae":["39",0],"prompt":request.prompt,"negative_prompt":request.negative_prompt,"resolution":0,"images.image_1":["77",0]}},
        "3":{"class_type":"KSampler","inputs":{"model":[cache,0],"positive":["64",0],"negative":["64",1],"latent_image":["62",0],"seed":request.seed,"steps":request.steps,"cfg":request.cfg,"sampler_name":"euler","scheduler":"simple","denoise":1.0}},
        "8":{"class_type":"VAEDecode","inputs":{"samples":["3",0],"vae":["39",0]}},
        "56":{"class_type":"SaveImage","inputs":{"images":["8",0],"filename_prefix":format!("PhotoCraft/{job}/result")}}
    });
    if let Some(object) = graph.as_object_mut() {
        object.insert(cache.into(), json!({"class_type":"QwenImage21Cache","inputs":{"model":["37",0],"device":"cpu","dtype":"int8"}}));
        if masked {
            object.insert("79".into(), json!({"class_type":"MaskToImage","inputs":{"mask":["78",1]}}));
            object.insert("80".into(), json!({"class_type":"ResizeImageMaskNode","inputs":{"input":["79",0],"resize_type":"match size","resize_type.match":["77",0],"resize_type.crop":"center","scale_method":"bicubic"}}));
            object.insert("83".into(), json!({"class_type":"VAEEncode","inputs":{"pixels":["77",0],"vae":["39",0]}}));
            // Pool coverage before thresholding: bilinear mask sampling can miss a thin stroke.
            object.insert("84".into(), json!({"class_type":"ImageScale","inputs":{"image":["79",0],"upscale_method":"area","width":(request.image.width/16).max(1),"height":(request.image.height/16).max(1),"crop":"disabled"}}));
            object.insert("85".into(), json!({"class_type":"ImageToMask","inputs":{"image":["84",0],"channel":"red"}}));
            object.insert("86".into(), json!({"class_type":"ThresholdMask","inputs":{"mask":["85",0],"value":0.0}}));
            object.insert("87".into(), json!({"class_type":"SetLatentNoiseMask","inputs":{"samples":["83",0],"mask":["86",0]}}));
            object.remove("62");
            if let Some(inputs) = object.get_mut("3").and_then(|n| n.get_mut("inputs")).and_then(Value::as_object_mut) {
                inputs.insert("latent_image".into(), json!(["87", 0]));
            }
            if let Some(inputs) = object.get_mut("64").and_then(|n| n.get_mut("inputs")).and_then(Value::as_object_mut) {
                inputs.insert("images.image_2".into(), json!(["80", 0]));
            }
        } else if request.image.pixels.as_chunks::<4>().0.iter().any(|p| p[3] < 255) {
            // LoadImage's MASK is 1-alpha, and JoinImageWithAlpha inverts it internally.
            object.insert("92".into(), json!({"class_type":"JoinImageWithAlpha","inputs":{"image":["78",0],"alpha":["78",1]}}));
            if let Some(inputs) = object.get_mut("77").and_then(|n| n.get_mut("inputs")).and_then(Value::as_object_mut) {
                inputs.insert("input".into(), json!(["92", 0]));
            }
        }
        if !masked && let (Some(reference), Some(filename)) = (&request.reference, reference_filename) {
            object.insert("93".into(), json!({"class_type":"LoadImage","inputs":{"image":filename}}));
            let transparent = reference.pixels.as_chunks::<4>().0.iter().any(|p| p[3] < 255);
            if transparent {
                object.insert("94".into(), json!({"class_type":"JoinImageWithAlpha","inputs":{"image":["93",0],"alpha":["93",1]}}));
            }
            object.insert("95".into(),json!({"class_type":"ResizeImageMaskNode","inputs":{"input":[if transparent{"94"}else{"93"},0],"resize_type":"scale to multiple","resize_type.multiple":32,"scale_method":"bicubic"}}));
            if let Some(inputs) = object.get_mut("64").and_then(|n| n.get_mut("inputs")).and_then(Value::as_object_mut) {
                inputs.insert("images.image_2".into(), json!(["95", 0]));
            }
        }
        if !masked
            && request.guide.is_some()
            && let Some(filename) = guide_filename
        {
            object.insert("96".into(), json!({"class_type":"LoadImage","inputs":{"image":filename}}));
            if let Some(inputs) = object.get_mut("64").and_then(|n| n.get_mut("inputs")).and_then(Value::as_object_mut) {
                inputs.insert("images.image_3".into(), json!(["96", 0]));
            }
        }
        if !masked && let (Some(sampling), Some((initial, noise))) = (&request.alpha_sampling, sampling_files) {
            object.insert("97".into(), json!({"class_type":"LoadImage","inputs":{"image":initial}}));
            object.insert("98".into(), json!({"class_type":"JoinImageWithAlpha","inputs":{"image":["97",0],"alpha":["97",1]}}));
            object.insert("83".into(), json!({"class_type":"VAEEncode","inputs":{"pixels":["98",0],"vae":["39",0]}}));
            object.insert("99".into(), json!({"class_type":"LoadImage","inputs":{"image":noise}}));
            object.insert("100".into(),json!({"class_type":"ImageScale","inputs":{"image":["99",0],"upscale_method":"area","width":(request.image.width/16).max(1),"height":(request.image.height/16).max(1),"crop":"disabled"}}));
            object.insert("101".into(), json!({"class_type":"ImageToMask","inputs":{"image":["100",0],"channel":"red"}}));
            object.insert("102".into(), json!({"class_type":"ThresholdMask","inputs":{"mask":["101",0],"value":0.0}}));
            object.insert("87".into(), json!({"class_type":"SetLatentNoiseMask","inputs":{"samples":["83",0],"mask":["102",0]}}));
            object.remove("62");
            if let Some(inputs) = object.get_mut("3").and_then(|n| n.get_mut("inputs")).and_then(Value::as_object_mut) {
                inputs.insert("latent_image".into(), json!(["87", 0]));
                inputs.insert("denoise".into(), json!(sampling.denoise));
            }
        }
    }
    turbo_sampler(&mut graph, request);
    graph
}

/// Resample the checkpoint's published eight-step curve for explicit user overrides.
fn turbo_sigmas(steps: u32) -> String {
    turbo_sigmas_tail(steps, 1.0)
}

fn turbo_sigmas_tail(steps: u32, denoise: f64) -> String {
    const SIGMAS: [f64; 9] = [1.0, 0.978453, 0.95418, 0.926626, 0.89508, 0.845148, 0.704534, 0.414568, 0.0];
    let steps = steps.clamp(1, crate::ai_cmds::MAX_STEPS);
    (0..=steps)
        .map(|i| {
            let start_position = 8.0 * (1.0 - denoise.clamp(0.0, 1.0));
            let position = start_position + f64::from(i) * (8.0 - start_position) / f64::from(steps);
            let index = position.floor() as usize;
            let start = SIGMAS.get(index).copied().unwrap_or(0.0);
            let end = SIGMAS.get(index.saturating_add(1)).copied().unwrap_or(start);
            format!("{:.6}", start + (end - start) * position.fract())
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn upload_image(client: &Client, base: &Url, image: &Rgba8Image, job: &str, name: &str, kind: &str) -> Result<String> {
    let data = Image::from_raw(image.width, image.height, ChannelLayout::Rgba, SampleType::U8, image.pixels.clone()).map_err(failure)?;
    let png = photocraft_codecs::encode(&data, Format::Png, &EncodeOptions::default()).map_err(failure)?;
    let part = multipart::Part::bytes(png).file_name(name.to_owned()).mime_str("image/png").map_err(failure)?;
    let form =
        multipart::Form::new().part("image", part).text("type", kind.to_owned()).text("subfolder", format!("PhotoCraft/{job}")).text("overwrite", "false");
    let url = base.join("upload/image").map_err(failure)?;
    let upload = read_json(client.post(url).timeout(Duration::from_secs(30)).multipart(form).send().map_err(failure)?)?;
    let name = upload.get("name").and_then(Value::as_str).filter(|s| !s.is_empty()).ok_or_else(|| failure("upload response has no image name"))?;
    let folder = upload.get("subfolder").and_then(Value::as_str).unwrap_or_default();
    let path = if folder.is_empty() { name.to_owned() } else { format!("{folder}/{name}") };
    Ok(if kind == "temp" { format!("{path} [temp]") } else { path })
}

fn turbo_sampler(graph: &mut Value, request: &AiRequest) {
    if !request.turbo && !request.model.is_turbo() {
        return;
    }
    let Some(inputs) = graph.get("3").and_then(|node| node.get("inputs")).and_then(Value::as_object).cloned() else { return };
    let Some(nodes) = graph.as_object_mut() else { return };
    nodes.insert("90".into(), json!({"class_type":"KSamplerSelect","inputs":{"sampler_name":"euler"}}));
    let sigmas = request.alpha_sampling.as_ref().map_or_else(|| turbo_sigmas(request.steps), |s| turbo_sigmas_tail(request.steps, s.denoise));
    nodes.insert("91".into(), json!({"class_type":"ManualSigmas","inputs":{"sigmas":sigmas}}));
    nodes.insert(
        "3".into(),
        json!({"class_type":"SamplerCustom","inputs":{
            "model":inputs.get("model"),"positive":inputs.get("positive"),"negative":inputs.get("negative"),"latent_image":inputs.get("latent_image"),
            "add_noise":true,"noise_seed":request.seed,"cfg":request.cfg,"sampler":["90",0],"sigmas":["91",0]
        }}),
    );
}

impl AiBackend for ComfyUi {
    fn unload_models(&self, endpoint: &str, ctx: &JobCtx) -> Result<()> {
        self.release_models(endpoint, ctx)
    }

    fn prepare_models(&self, request: &AiRequest, ctx: &JobCtx) -> Result<()> {
        let base = endpoint_url(&request.endpoint)?;
        let client = client(Duration::from_secs(3), Duration::from_secs(5))?;
        let models = model_inputs(&client, &base, ctx, request)?;
        self.image_models(&request.endpoint, &models, ctx)
    }
    fn available_translation_models(&self, endpoint: &str) -> Result<Vec<crate::prefs::ComfyUiTranslationModel>> {
        translation::models(endpoint)
    }

    fn translate(&self, endpoint: &str, model: crate::prefs::ComfyUiTranslationModel, prompt: &str, ctx: &JobCtx) -> Result<String> {
        self.translation_model(endpoint, model.filename(), ctx)?;
        translation::translate(endpoint, model, prompt, ctx)
    }

    fn check_available(&self, endpoint: &str) -> Result<()> {
        self.available_models(endpoint).map(|_| ())
    }

    fn available_models(&self, endpoint: &str) -> Result<Vec<ComfyUiModel>> {
        self.available_models_with_custom(endpoint, &[]).map(|(models, _)| models)
    }

    fn available_models_with_custom(&self, endpoint: &str, names: &[String]) -> Result<(Vec<ComfyUiModel>, Vec<String>)> {
        let config = crate::prefs::Integrations { comfyui_custom_models: names.join("\n"), ..Default::default() };
        let catalog = self.model_catalog(endpoint, &config)?;
        let mut models: Vec<_> = ComfyUiModel::ALL.into_iter().filter(|model| crate::ai_models::ready(&config, &catalog, model.filename())).collect();
        let custom: Vec<_> = names.iter().filter(|name| crate::ai_models::ready(&config, &catalog, name)).cloned().collect();
        if !custom.is_empty() {
            models.push(ComfyUiModel::Custom);
        }
        if models.is_empty() {
            return Err(failure("No model with an available text encoder"));
        }
        Ok((models, custom))
    }

    fn model_catalog(&self, endpoint: &str, config: &crate::prefs::Integrations) -> Result<crate::ai_models::Catalog> {
        let base = endpoint_url(endpoint)?;
        let client = client(Duration::from_secs(1), Duration::from_secs(2))?;
        let info = read_json(client.get(base.join("object_info/UNETLoader").map_err(failure)?).send().map_err(failure)?)?;
        let names = crate::prefs::custom_model_names(&config.comfyui_custom_models).map_err(failure)?;
        let mut models = Vec::new();
        for name in ComfyUiModel::ALL
            .into_iter()
            .map(|model| model.filename().to_owned())
            .chain(names.into_iter().filter(|name| crate::ai_models::builtin(name).is_none()))
        {
            if model_name(&info, "UNETLoader", "unet_name", &name).is_ok() && !models.contains(&name) {
                models.push(name);
            }
        }
        let clip = read_json(client.get(base.join("object_info/CLIPLoader").map_err(failure)?).send().map_err(failure)?)?;
        let encoders = detected_encoders(&clip, &crate::prefs::custom_model_names(&config.comfyui_custom_encoders).map_err(failure)?);
        let vae = read_json(client.get(base.join("object_info/VAELoader").map_err(failure)?).send().map_err(failure)?)?;
        model_name(&vae, "VAELoader", "vae_name", VAE_MODEL)?;
        for node in ["TextEncodeQwenImage21", "QwenImage21Cache"] {
            let info = read_json(client.get(base.join(&format!("object_info/{node}")).map_err(failure)?).send().map_err(failure)?)?;
            if !info.get(node).is_some_and(Value::is_object) {
                return Err(failure(format!("required node not found: {node}")));
            }
        }
        let mut turbo_available = true;
        if models.iter().any(|name| config.settings_for(name).turbo) {
            for node in ["SamplerCustom", "ManualSigmas", "KSamplerSelect"] {
                let info = read_json(client.get(base.join(&format!("object_info/{node}")).map_err(failure)?).send().map_err(failure)?)?;
                if !info.get(node).is_some_and(Value::is_object) {
                    turbo_available = false;
                    break;
                }
            }
        }
        Ok(crate::ai_models::Catalog { models, encoders, turbo_available })
    }

    fn resolve_text_encoder(&self, endpoint: &str, configured: Option<&str>, registered: &[String]) -> Result<String> {
        let base = endpoint_url(endpoint)?;
        let client = client(Duration::from_secs(1), Duration::from_secs(2))?;
        let info = read_json(client.get(base.join("object_info/CLIPLoader").map_err(failure)?).send().map_err(failure)?)?;
        crate::ai_models::resolve_encoder(configured, &detected_encoders(&info, registered)).map_err(failure)
    }

    fn generate(&self, request: &AiRequest, ctx: &JobCtx) -> Result<Rgba8Image> {
        for (reference, budget) in request
            .reference
            .iter()
            .map(|image| (image, crate::ai_cmds::resolution::Budget::ATTACHED_REFERENCE))
            .chain(request.guide.iter().map(|image| (image, crate::ai_cmds::resolution::Budget::REFERENCE)))
        {
            if request.mode != EditMode::PromptEdit
                || !budget.accepts_frame([reference.width, reference.height])
                || reference.pixels.len() != reference.width as usize * reference.height as usize * 4
            {
                return Err(failure("invalid reference image frame"));
            }
        }
        if request.guide.is_some() && request.reference.is_none() {
            return Err(failure("a refinement guide requires global context"));
        }
        if let Some(sampling) = &request.alpha_sampling {
            if request.mode != EditMode::PromptEdit
                || request.reference.is_none()
                || request.guide.is_none()
                || !sampling.denoise.is_finite()
                || sampling.denoise <= 0.0
                || sampling.denoise > 1.0
                || !crate::ai_cmds::resolution::Budget::EDIT.accepts_frame([request.image.width, request.image.height])
            {
                return Err(failure("invalid alpha sampler configuration"));
            }
            for image in [&sampling.initial, &sampling.noise] {
                if image.width != request.image.width
                    || image.height != request.image.height
                    || image.pixels.len() != image.width as usize * image.height as usize * 4
                {
                    return Err(failure("alpha sampler images must match the primary frame"));
                }
            }
            if sampling.noise.pixels.as_chunks::<4>().0.iter().any(|p| p[0] != p[1] || p[1] != p[2] || p[3] != 255) {
                return Err(failure("alpha sampling coverage must be opaque grayscale"));
            }
        }
        if request.mode == EditMode::GenerativeFill
            && (request.image.width == 0
                || request.image.height == 0
                || !crate::ai_cmds::resolution::Budget::EDIT.accepts_frame([request.image.width, request.image.height])
                || !request.image.width.is_multiple_of(32)
                || !request.image.height.is_multiple_of(32))
        {
            return Err(failure("selection fill requires a nonempty 32-padded image within the Qwen pixel budget"));
        }
        let base = endpoint_url(&request.endpoint)?;
        let route = |path: &str| base.join(path).map_err(failure);
        let client = client(Duration::from_secs(3), Duration::from_secs(5))?;
        ctx.check()?;
        let models = model_inputs(&client, &base, ctx, request)?;
        self.image_models(&request.endpoint, &models, ctx)?;
        ctx.check()?;
        let job = uuid::Uuid::new_v4().to_string();
        let filename = if request.mode == EditMode::PromptGenerate {
            String::new()
        } else {
            ctx.progress(0.1, "Uploading image to ComfyUI");
            upload_image(&client, &base, &request.image, &job, "source.png", "input")?
        };
        let reference_filename = if let Some(reference) = &request.reference {
            ctx.check()?;
            ctx.progress(0.13, "Uploading reference image");
            Some(upload_image(&client, &base, reference, &job, "reference.png", "temp")?)
        } else {
            None
        };
        let guide_filename = if let Some(guide) = &request.guide {
            ctx.check()?;
            Some(upload_image(&client, &base, guide, &job, "guide.png", "temp")?)
        } else {
            None
        };
        let sampling_files = if let Some(sampling) = &request.alpha_sampling {
            ctx.check()?;
            let initial = upload_image(&client, &base, &sampling.initial, &job, "initial.png", "temp")?;
            ctx.check()?;
            let noise = upload_image(&client, &base, &sampling.noise, &job, "noise.png", "temp")?;
            Some((initial, noise))
        } else {
            None
        };
        ctx.check()?;
        ctx.progress(0.2, "Waiting for ComfyUI generation");
        // Subscribe before POST so even short/cached executions cannot outrun the client.
        let mut live = progress::Live::connect(&base, &job);
        let mut prompt = workflow_with_sampling(
            request,
            &filename,
            reference_filename.as_deref(),
            guide_filename.as_deref(),
            sampling_files.as_ref().map(|(a, b)| (a.as_str(), b.as_str())),
            &job,
        );
        for (id, input, name) in models {
            let value = prompt.pointer_mut(&format!("/{id}/inputs/{input}")).ok_or_else(|| failure("workflow model input missing"))?;
            *value = Value::String(name);
        }
        // A client-selected UUID lets us cancel our job even when the POST reply is lost.
        // POST is never blindly retried: repeating the UUID does not guarantee deduplication.
        let outcome = (|| {
            ctx.check()?;
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
                live.poll(&job, ctx)?;
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
        image.data().as_chunks::<3>().0.iter().flat_map(|p| p.iter().copied().chain(std::iter::once(255))).collect()
    };
    Ok(Rgba8Image { width, height, pixels })
}

#[cfg(test)]
mod tests;
