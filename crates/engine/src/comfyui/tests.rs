use super::*;

fn request(mode: EditMode) -> AiRequest {
    AiRequest {
        image: Rgba8Image { width: 32, height: 32, pixels: vec![255; 32 * 32 * 4] },
        prompt: "plain prompt".into(),
        mode,
        endpoint: "http://127.0.0.1:8188".into(),
        seed: 1234,
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
    assert_eq!(masked["64"]["inputs"]["images.image_2"], json!(["80", 0]));
    assert_eq!(masked["82"]["inputs"]["dtype"], "int8");
    assert!(masked.get("81").is_none());
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

fn mock_server(cancel: Option<JobCtx>) -> (String, std::thread::JoinHandle<Vec<String>>) {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut paths = Vec::new();
        let mut job = String::new();
        while Instant::now() < deadline {
            let Ok((stream, _)) = listener.accept() else {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            };
            stream.set_nonblocking(false).unwrap();
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
            let (mime, response) = if path == "/upload/image" {
                assert!(String::from_utf8_lossy(&body).contains("source.png"));
                ("application/json", serde_json::to_vec(&json!({"name":"source.png","subfolder":"mock","type":"input"})).unwrap())
            } else if path == "/prompt" {
                let submitted: Value = serde_json::from_slice(&body).unwrap();
                job = submitted["prompt_id"].as_str().unwrap().into();
                assert_eq!(submitted["prompt"]["64"]["inputs"]["prompt"], "plain prompt");
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
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", response.len()).unwrap();
            stream.write_all(&response).unwrap();
            stream.flush().unwrap();
            let finished = path.starts_with("/view?") || path.ends_with("/cancel");
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
    let (endpoint, server) = mock_server(None);
    let mut request = request(EditMode::PromptEdit);
    request.endpoint = endpoint;
    let result = ComfyUi.generate(&request, &JobCtx::new()).unwrap();
    assert_eq!(result.pixels.get(..4), Some([80, 80, 80, 255].as_slice()));
    let paths = server.join().unwrap();
    assert!(paths.len() >= 4);
    assert!(paths.last().unwrap().starts_with("/view?"));
}

#[test]
fn cancellation_targets_only_the_submitted_job() {
    let ctx = JobCtx::new();
    let (endpoint, server) = mock_server(Some(ctx.clone()));
    let mut request = request(EditMode::GenerativeFill);
    request.endpoint = endpoint;
    assert!(matches!(ComfyUi.generate(&request, &ctx), Err(EngineError::Cancelled)));
    let paths = server.join().unwrap();
    assert!(paths.len() >= 4);
    assert!(paths.last().unwrap().ends_with("/cancel"));
    assert!(!paths.iter().any(|p| p == "/interrupt" || p == "/queue"));
}
