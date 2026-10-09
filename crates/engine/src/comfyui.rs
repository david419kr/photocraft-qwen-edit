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
const CLIP_MODEL: &str = "qwen3vl_8b_int8_convrot.safetensors";
const VAE_MODEL: &str = "qwen_image_2.1_vae_bf16.safetensors";

#[derive(Default)]
pub struct ComfyUi;

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
    for (id, node, input, filename) in
        [("37", "UNETLoader", "unet_name", model.filename()), ("38", "CLIPLoader", "clip_name", CLIP_MODEL), ("39", "VAELoader", "vae_name", VAE_MODEL)]
    {
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
    if request.mode == EditMode::PromptGenerate {
        return json!({
            "37":{"class_type":"UNETLoader","inputs":{"unet_name":request.model.filename(),"weight_dtype":"default"}},
            "38":{"class_type":"CLIPLoader","inputs":{"clip_name":CLIP_MODEL,"type":"qwen_image","device":"default"}},
            "39":{"class_type":"VAELoader","inputs":{"vae_name":VAE_MODEL}},
            "6":{"class_type":"CLIPTextEncode","inputs":{"clip":["38",0],"text":request.prompt}},
            "63":{"class_type":"ConditioningZeroOut","inputs":{"conditioning":["6",0]}},
            "62":{"class_type":"EmptyLatentImage","inputs":{"width":request.image.width,"height":request.image.height,"batch_size":1}},
            "3":{"class_type":"KSampler","inputs":{"model":["37",0],"positive":["6",0],"negative":["63",0],"latent_image":["62",0],"seed":request.seed,"steps":25,"cfg":1.0,"sampler_name":"euler","scheduler":"simple","denoise":1.0}},
            "8":{"class_type":"VAEDecode","inputs":{"samples":["3",0],"vae":["39",0]}},
            "56":{"class_type":"SaveImage","inputs":{"images":["8",0],"filename_prefix":format!("PhotoCraft/{job}/result")}}
        });
    }
    let masked = request.mode == EditMode::GenerativeFill;
    let cache = if masked { "82" } else { "79" };
    let mut graph = json!({
        "37":{"class_type":"UNETLoader","inputs":{"unet_name":request.model.filename(),"weight_dtype":"default"}},
        "38":{"class_type":"CLIPLoader","inputs":{"clip_name":CLIP_MODEL,"type":"qwen_image","device":"default"}},
        "39":{"class_type":"VAELoader","inputs":{"vae_name":VAE_MODEL}},
        "78":{"class_type":"LoadImage","inputs":{"image":filename}},
        "77":{"class_type":"ResizeImageMaskNode","inputs":{"input":["78",0],"resize_type":"scale to multiple","resize_type.multiple":32,"scale_method":"bicubic"}},
        "76":{"class_type":"GetImageSize","inputs":{"image":["77",0]}},
        "62":{"class_type":"EmptyLatentImage","inputs":{"width":["76",0],"height":["76",1],"batch_size":1}},
        "64":{"class_type":"TextEncodeQwenImage21","inputs":{"clip":["38",0],"vae":["39",0],"prompt":request.prompt,"negative_prompt":"","resolution":0,"images.image_1":["77",0]}},
        "3":{"class_type":"KSampler","inputs":{"model":[cache,0],"positive":["64",0],"negative":["64",1],"latent_image":["62",0],"seed":request.seed,"steps":25,"cfg":1.0,"sampler_name":"euler","scheduler":"simple","denoise":1.0}},
        "8":{"class_type":"VAEDecode","inputs":{"samples":["3",0],"vae":["39",0]}},
        "56":{"class_type":"SaveImage","inputs":{"images":["8",0],"filename_prefix":format!("PhotoCraft/{job}/result")}}
    });
    if let Some(object) = graph.as_object_mut() {
        object.insert(cache.into(), json!({"class_type":"QwenImage21Cache","inputs":{"model":["37",0],"device":"auto","dtype":"int8"}}));
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
    graph
}

impl AiBackend for ComfyUi {
    fn check_available(&self, endpoint: &str) -> Result<()> {
        self.available_models(endpoint).map(|_| ())
    }

    fn available_models(&self, endpoint: &str) -> Result<Vec<ComfyUiModel>> {
        let base = endpoint_url(endpoint)?;
        let client = client(Duration::from_secs(1), Duration::from_secs(2))?;
        let info = read_json(client.get(base.join("object_info/UNETLoader").map_err(failure)?).send().map_err(failure)?)?;
        let models: Vec<_> = ComfyUiModel::ALL.into_iter().filter(|m| model_name(&info, "UNETLoader", "unet_name", m.filename()).is_ok()).collect();
        if models.is_empty() {
            return Err(failure("no supported Qwen Image 2.1 or NoctQ V4 checkpoint found"));
        }
        for (node, input, filename) in [("CLIPLoader", "clip_name", CLIP_MODEL), ("VAELoader", "vae_name", VAE_MODEL)] {
            let info = read_json(client.get(base.join(&format!("object_info/{node}")).map_err(failure)?).send().map_err(failure)?)?;
            model_name(&info, node, input, filename)?;
        }
        for node in ["TextEncodeQwenImage21", "QwenImage21Cache"] {
            let info = read_json(client.get(base.join(&format!("object_info/{node}")).map_err(failure)?).send().map_err(failure)?)?;
            if !info.get(node).is_some_and(Value::is_object) {
                return Err(failure(format!("required node not found: {node}")));
            }
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
