use super::*;

#[test]
fn cfg_negative_encoder_and_custom_turbo_are_carried_to_every_workflow() {
    for mode in [EditMode::PromptGenerate, EditMode::PromptEdit, EditMode::GenerativeFill] {
        let mut input = request(mode);
        input.cfg = 3.5;
        input.text_encoder = "user_te.safetensors".into();
        input.negative_prompt = "blur, noise".into();
        let graph = workflow(&input, "source.png", "test");
        assert_eq!(graph["38"]["inputs"]["clip_name"], "user_te.safetensors");
        assert_eq!(graph["3"]["inputs"]["cfg"], 3.5);
        if mode == EditMode::PromptGenerate {
            assert_eq!(graph["63"]["inputs"]["text"], "blur, noise");
        } else {
            assert_eq!(graph["64"]["inputs"]["negative_prompt"], "blur, noise");
        }
        input.model = ComfyUiModel::Custom;
        input.custom_model = Some("custom_turbo.safetensors".into());
        input.turbo = true;
        assert_eq!(workflow(&input, "source.png", "test")["3"]["class_type"], "SamplerCustom");
    }
}

#[test]
fn custom_checkpoint_reuses_all_qwen_workflows_without_prompt_changes() {
    for mode in [EditMode::PromptGenerate, EditMode::PromptEdit, EditMode::GenerativeFill] {
        let builtin = request(mode);
        let mut custom = builtin.clone();
        custom.model = ComfyUiModel::Custom;
        custom.custom_model = Some("folder/user_model.safetensors".into());
        let mut graph = workflow(&custom, "source.png", "output");
        assert_eq!(graph["37"]["class_type"], "UNETLoader");
        assert_eq!(graph["37"]["inputs"]["unet_name"], "folder/user_model.safetensors");
        graph["37"]["inputs"]["unet_name"] = json!(builtin.model.filename());
        assert_eq!(graph, workflow(&builtin, "source.png", "output"));
        let info = json!({"UNETLoader":{"input":{"required":{"unet_name":[["nested/user_model.safetensors"]]}}}});
        assert_eq!(model_name(&info, "UNETLoader", "unet_name", "user_model.safetensors").unwrap(), "nested/user_model.safetensors");
        assert_eq!(model_name(&info, "UNETLoader", "unet_name", "nested\\user_model.safetensors").unwrap(), "nested/user_model.safetensors");
    }
}

fn request(mode: EditMode) -> AiRequest {
    AiRequest {
        model: ComfyUiModel::default(),
        custom_model: None,
        text_encoder: crate::ai_models::DEFAULT_ENCODER.into(),
        cfg: 1.0,
        negative_prompt: String::new(),
        turbo: false,
        image: Rgba8Image { width: 32, height: 32, pixels: vec![255; 32 * 32 * 4] },
        prompt: "plain prompt".into(),
        mode,
        removal: false,
        endpoint: "http://127.0.0.1:8188".into(),
        seed: 1234,
        steps: 25,
    }
}

#[test]
fn supported_models_use_the_same_edit_and_fill_graph_and_resolve_nested_paths() {
    for model in ComfyUiModel::ALL {
        for mode in [EditMode::PromptEdit, EditMode::GenerativeFill] {
            let mut input = request(mode);
            input.model = model;
            let graph = workflow(&input, "source.png", "models");
            assert_eq!(graph["37"]["inputs"]["unet_name"], model.filename());
            assert_eq!(graph["38"]["inputs"]["clip_name"], model.clip_filename());
            assert_eq!(graph["39"]["inputs"]["vae_name"], VAE_MODEL);
        }
        let nested = format!("Qwen/{}", model.filename());
        let info = json!({"UNETLoader":{"input":{"required":{"unet_name":[[nested],{}]}}}});
        assert_eq!(model_name(&info, "UNETLoader", "unet_name", model.filename()).unwrap(), nested);
    }
}

#[test]
fn exact_qwen_graph_and_mask_wiring() {
    let global = workflow(&request(EditMode::PromptEdit), "source.png", "test");
    assert_eq!(global["64"]["inputs"]["prompt"], "plain prompt");
    assert_eq!(global["64"]["inputs"]["resolution"], 0);
    assert!(global["64"]["inputs"].get("images.image_2").is_none());
    assert_eq!(global["79"]["class_type"], "QwenImage21Cache");
    let masked = workflow(&request(EditMode::GenerativeFill), "source.png", "test");
    assert_eq!(masked["79"]["class_type"], "MaskToImage");
    assert_eq!(masked["64"]["inputs"]["images.image_1"], json!(["77", 0]));
    assert_eq!(masked["64"]["inputs"]["images.image_2"], json!(["80", 0]));
    assert_eq!(masked["82"]["inputs"]["dtype"], "int8");
    assert!(masked.get("81").is_none());
    let mut removal = request(EditMode::GenerativeFill);
    removal.removal = true;
    let annotated = workflow(&removal, "source.png", "test");
    assert_eq!(annotated["79"]["class_type"], "EmptyImage");
    assert_eq!(annotated["79"]["inputs"]["color"], 65280);
    assert_eq!(annotated["80"]["class_type"], "ImageCompositeMasked");
    assert_eq!(annotated["80"]["inputs"]["destination"], json!(["77", 0]));
    assert_eq!(annotated["80"]["inputs"]["mask"], json!(["78", 1]));
    assert_eq!(annotated["64"]["inputs"]["images.image_1"], json!(["80", 0]));
    assert!(annotated["64"]["inputs"].get("images.image_2").is_none());
}

#[test]
fn turbo_loader_schedule_and_steps_preserve_existing_prompts_and_masks() {
    let expected = "1.000000,0.978453,0.954180,0.926626,0.895080,0.845148,0.704534,0.414568,0.000000";
    assert_eq!(turbo_sigmas(8), expected);
    for model in ComfyUiModel::ALL.into_iter().filter(|model| model.is_turbo()) {
        for mode in [EditMode::PromptGenerate, EditMode::PromptEdit, EditMode::GenerativeFill] {
            let mut input = request(mode);
            input.model = model;
            input.steps = model.default_steps();
            let graph = workflow(&input, "source.png", "turbo");
            assert_eq!(input.steps, 8);
            assert_eq!(graph["37"]["class_type"], model.loader());
            assert_eq!(graph["3"]["class_type"], "SamplerCustom");
            assert_eq!(graph["3"]["inputs"]["cfg"], 1.0);
            assert_eq!(graph["3"]["inputs"]["noise_seed"], input.seed);
            assert_eq!(graph["91"]["inputs"]["sigmas"], expected);
            if mode != EditMode::PromptGenerate {
                assert_eq!(graph["64"]["inputs"]["prompt"], input.prompt);
                assert_eq!(graph["64"]["inputs"]["negative_prompt"], "");
                let cache = if mode == EditMode::GenerativeFill { "82" } else { "79" };
                assert_eq!(graph[cache]["inputs"]["dtype"], if model.is_gguf() { "default" } else { "int8" });
            }
            input.steps = 4;
            let custom = workflow(&input, "source.png", "custom");
            let sigmas: Vec<f64> = custom["91"]["inputs"]["sigmas"].as_str().unwrap().split(',').map(|s| s.parse().unwrap()).collect();
            assert_eq!(sigmas.len(), 5);
            assert_eq!(sigmas.first(), Some(&1.0));
            assert_eq!(sigmas.last(), Some(&0.0));
            assert!(sigmas.windows(2).all(|pair| pair[0] >= pair[1]));
        }
    }
    let (endpoint, server) = mock_server_model(None, Some(true), ComfyUiModel::TurboInt8);
    assert_eq!(ComfyUi.available_models(&endpoint).unwrap(), vec![ComfyUiModel::TurboInt8]);
    assert_eq!(server.join().unwrap().len(), 8);
}

#[test]
fn model_lookup_accepts_subfolders_and_preserves_root_preference() {
    let info = |names: Value| json!({"UNETLoader":{"input":{"required":{"unet_name":[names,{}]}}}});
    for path in [format!("Qwen-Image-2.1/{UNET_MODEL}"), format!("models\\Qwen\\{UNET_MODEL}")] {
        assert_eq!(model_name(&info(json!([path])), "UNETLoader", "unet_name", UNET_MODEL).unwrap(), path);
    }
    let nested = format!("Qwen/{UNET_MODEL}");
    assert_eq!(model_name(&info(json!([nested, UNET_MODEL])), "UNETLoader", "unet_name", UNET_MODEL).unwrap(), UNET_MODEL);
    assert_eq!(model_name(&info(json!([nested, nested])), "UNETLoader", "unet_name", UNET_MODEL).unwrap(), nested);
    assert!(model_name(&info(json!(["qwen_image_2.1_bf16.safetensors"])), "UNETLoader", "unet_name", UNET_MODEL).is_err());
    assert!(
        model_name(&info(json!([nested, format!("other/{UNET_MODEL}")])), "UNETLoader", "unet_name", UNET_MODEL)
            .unwrap_err()
            .to_string()
            .contains("multiple models")
    );
    assert!(model_name(&json!({}), "UNETLoader", "unet_name", UNET_MODEL).is_err());
    assert!(model_name(&info(json!(false)), "UNETLoader", "unet_name", UNET_MODEL).is_err());
}

#[test]
fn reject_remote_urls_and_oversized_outputs_before_use() {
    for endpoint in ["http://example.com:8188", "file:///x", "http://user:password@localhost:8188", "http://127.0.0.1:8188/?token=secret"] {
        assert!(endpoint_url(endpoint).is_err());
    }
    assert!(endpoint_url("http://[::1]:8188").is_ok());
    let image = Image::from_raw(64, 64, ChannelLayout::Rgba, SampleType::U8, vec![255; 64 * 64 * 4]).unwrap();
    let png = photocraft_codecs::encode(&image, Format::Png, &EncodeOptions::default()).unwrap();
    assert!(decode_result(&png, 32, 32).is_err());
}

fn mock_server(cancel: Option<JobCtx>, probe: Option<bool>) -> (String, std::thread::JoinHandle<Vec<String>>) {
    mock_server_model(cancel, probe, ComfyUiModel::QwenImage21)
}

fn mock_server_model(cancel: Option<JobCtx>, probe: Option<bool>, selected: ComfyUiModel) -> (String, std::thread::JoinHandle<Vec<String>>) {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut paths = Vec::new();
        let mut job = String::new();
        while Instant::now() < deadline {
            let Ok((stream, _)) = listener.accept() else {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            };
            stream.set_nonblocking(false).unwrap();
            stream.set_nodelay(true).unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            let mut reader = BufReader::new(stream);
            let mut first = String::new();
            reader.read_line(&mut first).unwrap();
            let path = first.split_whitespace().nth(1).unwrap().to_owned();
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some(n) = line.to_lowercase().strip_prefix("content-length:") {
                    length = n.trim().parse::<usize>().unwrap();
                }
            }
            assert!(length < 1 << 20);
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let (mime, response) = if let Some(node) = path.strip_prefix("/object_info/") {
                let (input, filename) = match node {
                    "UNETLoader" | "UnetLoaderGGUF" => ("unet_name", if selected.loader() == node { selected.filename() } else { "missing-model" }),
                    "CLIPLoader" => ("clip_name", selected.clip_filename()),
                    "VAELoader" => ("vae_name", VAE_MODEL),
                    "TextEncodeQwenImage21" | "QwenImage21Cache" | "SamplerCustom" | "ManualSigmas" | "KSamplerSelect" => ("model", "unused"),
                    _ => panic!("unexpected model list request {path}"),
                };
                let filename = if probe == Some(false) { "missing.safetensors" } else { filename };
                ("application/json", serde_json::to_vec(&json!({node:{"input":{"required":{input:[[format!("Qwen/{filename}")],{}]}}}})).unwrap())
            } else if path == "/upload/image" {
                assert!(String::from_utf8_lossy(&body).contains("source.png"));
                ("application/json", serde_json::to_vec(&json!({"name":"source.png","subfolder":"mock","type":"input"})).unwrap())
            } else if path == "/prompt" {
                let submitted: Value = serde_json::from_slice(&body).unwrap();
                job = submitted["prompt_id"].as_str().unwrap().into();
                if submitted["prompt"].get("6").is_some() {
                    assert_eq!(submitted["prompt"]["6"]["inputs"]["text"], "plain prompt");
                    assert!(submitted["prompt"].get("78").is_none());
                } else {
                    assert_eq!(submitted["prompt"]["64"]["inputs"]["prompt"], "plain prompt");
                }
                assert_eq!(submitted["prompt"]["37"]["inputs"]["unet_name"], format!("Qwen/{}", selected.filename()));
                assert_eq!(submitted["prompt"]["38"]["inputs"]["clip_name"], format!("Qwen/{}", selected.clip_filename()));
                assert_eq!(submitted["prompt"]["39"]["inputs"]["vae_name"], format!("Qwen/{VAE_MODEL}"));
                ("application/json", serde_json::to_vec(&json!({"prompt_id":job,"number":1,"node_errors":{}})).unwrap())
            } else if path.starts_with("/api/jobs/") {
                assert_eq!(path, format!("/api/jobs/{job}/cancel"));
                ("application/json", serde_json::to_vec(&json!({"cancelled": true})).unwrap())
            } else if path.starts_with("/history/") && cancel.is_some() {
                cancel.as_ref().unwrap().cancel();
                ("application/json", serde_json::to_vec(&json!({})).unwrap())
            } else if path.starts_with("/history/") {
                ("application/json",serde_json::to_vec(&json!({job.clone():{"outputs":{"81":{"images":[{"filename":"wrong-mask.png"}]},"56":{"images":[{"filename":"result.png","subfolder":"mock","type":"output"}]}},"status":{"completed":true,"status_str":"success"}}})).unwrap())
            } else {
                assert!(path.starts_with("/view?"), "unexpected path {path}; earlier paths: {paths:?}");
                assert!(path.contains("result.png"));
                let image = Image::from_raw(32, 32, ChannelLayout::Rgb, SampleType::U8, vec![80; 32 * 32 * 3]).unwrap();
                ("image/png", photocraft_codecs::encode(&image, Format::Png, &EncodeOptions::default()).unwrap())
            };
            let mut stream = reader.into_inner();
            let mut wire = format!("HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", response.len()).into_bytes();
            wire.extend_from_slice(&response);
            stream.write_all(&wire).unwrap();
            stream.flush().unwrap();
            // Finish the HTTP connection gracefully on Windows rather than resetting a peer
            // that is still receiving the response when this fixture drops its socket.
            stream.shutdown(std::net::Shutdown::Write).unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let _ = std::io::Read::by_ref(&mut stream).take(8192).read_to_end(&mut Vec::new());
            let finished = path.starts_with("/view?")
                || path.ends_with("/cancel")
                || probe == Some(false) && path == "/object_info/VAELoader"
                || probe == Some(true) && path == if selected.is_turbo() { "/object_info/KSamplerSelect" } else { "/object_info/QwenImage21Cache" };
            paths.push(path);
            if finished {
                break;
            }
        }
        paths
    });
    (endpoint, server)
}

#[test]
fn round_trip_through_a_mock_server_never_uses_a_real_model() {
    let (endpoint, server) = mock_server(None, None);
    let mut request = request(EditMode::PromptEdit);
    request.endpoint = endpoint;
    let result = ComfyUi.generate(&request, &JobCtx::new());
    let paths = server.join().unwrap();
    let result = result.unwrap_or_else(|error| panic!("{error:?}; served paths: {paths:?}"));
    assert_eq!(result.pixels.get(..4), Some([80, 80, 80, 255].as_slice()));
    assert!(paths.len() >= 7);
    assert!(paths.iter().take(3).all(|path| path.starts_with("/object_info/")));
    assert!(paths.last().unwrap().starts_with("/view?"));
}

#[test]
fn text_to_image_matches_workflow_one_and_never_uploads_source_image() {
    for model in ComfyUiModel::ALL {
        let mut input = request(EditMode::PromptGenerate);
        input.model = model;
        input.steps = model.default_steps();
        let graph = workflow(&input, "unused.png", "t2i");
        assert_eq!(graph["6"]["class_type"], "CLIPTextEncode");
        assert_eq!(graph["63"]["class_type"], "ConditioningZeroOut");
        assert_eq!(graph["3"]["inputs"]["model"], json!(["37", 0]));
        assert!(graph.get("78").is_none());
        assert_eq!(graph["62"]["inputs"]["width"], 32);
        let (endpoint, server) = mock_server_model(None, None, model);
        input.endpoint = endpoint;
        assert!(ComfyUi.generate(&input, &JobCtx::new()).is_ok());
        assert!(!server.join().unwrap().iter().any(|p| p == "/upload/image"));
    }
}

#[test]
fn cancellation_targets_only_the_submitted_job() {
    let ctx = JobCtx::new();
    let (endpoint, server) = mock_server(Some(ctx.clone()), None);
    let mut request = request(EditMode::GenerativeFill);
    request.endpoint = endpoint;
    let result = ComfyUi.generate(&request, &ctx);
    let paths = server.join().unwrap();
    assert!(matches!(result, Err(EngineError::Cancelled)), "{result:?}; served paths: {paths:?}");
    assert!(paths.len() >= 4);
    assert!(paths.last().unwrap().ends_with("/cancel"));
    assert!(!paths.iter().any(|p| p == "/interrupt" || p == "/queue"));
}

#[test]
fn availability_checks_models_and_nodes_without_upload_or_generation() {
    for present in [true, false] {
        let (endpoint, server) = mock_server(None, Some(present));
        let available = ComfyUi.check_available(&endpoint);
        let paths = server.join().unwrap();
        assert_eq!(available.is_ok(), present, "{available:?}");
        assert!(paths.iter().all(|path| path.starts_with("/object_info/")), "{paths:?}");
        assert_eq!(paths.len(), if present { 5 } else { 3 });
    }
}
