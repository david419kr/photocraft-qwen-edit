use super::*;
use crate::comfyui::{ComfyUi, EngineError, Instant, endpoint_url, json};

#[test]
#[ignore = "manual existing-ComfyUI integration; set PHOTOCRAFT_PROGRESS_PROBE_OUTPUT"]
fn real_progress_translation_release_and_2k_generation() {
    use crate::{Session, prefs::ComfyUiModel};
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };
    let out = std::env::var("PHOTOCRAFT_PROGRESS_PROBE_OUTPUT").expect("Explicit output directory required");
    let mut s = Session::new();
    s.ai_backend = Some(Arc::new(ComfyUi::default()));
    s.execute("prefs.set", json!({"path":"integrations.comfyuiModel","value":"turboInt8"})).unwrap();
    s.execute("prefs.set", json!({"path":"integrations.comfyuiTranslatePrompt","value":true})).unwrap();
    s.execute("prefs.set", json!({"path":"integrations.comfyuiTranslationModel","value":"hyMt18Heretic"})).unwrap();
    s.execute("file.new", json!({"width":2048,"height":2048,"background":"#788c90"})).unwrap();
    let ctx = JobCtx::new();
    s.set_inline_job_ctx(Some(ctx.clone()));
    let events = Arc::new(Mutex::new(Vec::<Value>::new()));
    let finished = Arc::new(AtomicBool::new(false));
    let begin = Instant::now();
    let (capture, done, watch) = (events.clone(), finished.clone(), ctx.clone());
    let thread = std::thread::spawn(move || {
        let mut last = None;
        while !done.load(Ordering::Relaxed) {
            let current = (watch.fraction().to_bits(), watch.message());
            if last.as_ref() != Some(&current) {
                capture.lock().unwrap().push(json!({"ms":begin.elapsed().as_millis(),"fraction":watch.fraction(),"message":current.1}));
                last = Some(current);
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    });
    let result = s.execute("edit.promptEdit", json!({"prompt":"가운데에 작은 흰색 원 하나를 추가해 주세요.","steps":4,"seed":2026102804u64}));
    finished.store(true, Ordering::Relaxed);
    thread.join().unwrap();
    std::fs::create_dir_all(&out).unwrap();
    let events = events.lock().unwrap();
    let report = json!({"version":crate::build_info::release_tag(),"seconds":begin.elapsed().as_secs_f64(),"model":ComfyUiModel::TurboInt8.filename(),"result":result.as_ref().map_err(ToString::to_string),"events":*events});
    std::fs::write(std::path::Path::new(&out).join("result.json"), serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    result.unwrap();
    let sampler: Vec<_> = events.iter().filter(|v| v["message"].as_str().is_some_and(|m| m.contains("Generating image ·"))).collect();
    assert!(sampler.len() >= 2, "Expected multiple real sampling updates: {report}");
    assert!(sampler.iter().any(|v| v["fraction"].as_f64().is_some_and(|p| p > 0.3 && p < 0.85)));
    println!("{}", json!({"seconds":begin.elapsed().as_secs_f64(),"samplingUpdates":sampler.len(),"version":crate::build_info::release_tag()}));
}

#[test]
fn real_steps_map_monotonically_and_ignore_other_jobs_nodes_and_malformed_data() {
    let ctx = JobCtx::new();
    for event in [
        json!({"type":"progress","data":{"prompt_id":"other","node":"3","value":10,"max":10}}),
        json!({"type":"progress","data":{"prompt_id":"ours","node":"64","value":10,"max":10}}),
        json!({"type":"progress","data":{"prompt_id":"ours","node":"3","value":1,"max":0}}),
        json!({"type":"progress","data":{"prompt_id":"ours","node":"3","value":-1,"max":4}}),
        json!([]),
    ] {
        apply(&event, "ours", &ctx);
    }
    assert_eq!(ctx.fraction(), 0.0);
    apply(&json!({"type":"progress","data":{"prompt_id":"ours","node":"3","value":1,"max":4}}), "ours", &ctx);
    assert!((ctx.fraction() - 0.3625).abs() < 1e-6);
    assert!(ctx.message().contains("1/4 (25%)"));
    apply(&json!({"type":"progress_state","data":{"prompt_id":"ours","nodes":{"3":{"prompt_id":"ours","value":3,"max":4,"state":"running"}}}}), "ours", &ctx);
    assert!((ctx.fraction() - 0.6875).abs() < 1e-6);
    apply(&json!({"type":"progress_state","data":{"prompt_id":"ours","nodes":{"3":{"prompt_id":"other","value":4,"max":4}}}}), "ours", &ctx);
    assert!((ctx.fraction() - 0.6875).abs() < 1e-6);
    apply(&json!({"type":"progress","data":{"prompt_id":"ours","node":"3","value":2,"max":4}}), "ours", &ctx);
    assert!((ctx.fraction() - 0.6875).abs() < 1e-6);
    apply(
        &json!({"type":"progress_state","data":{"prompt_id":"ours","nodes":{"3":{"value":4,"max":4,"state":"finished"},"8":{"value":1,"max":2,"state":"running"}}}}),
        "ours",
        &ctx,
    );
    assert!((ctx.fraction() - 0.875).abs() < 1e-6);
    assert!(ctx.fraction() < 1.0);
}

#[test]
fn scoped_batch_progress_and_cached_execution_keep_the_same_job_contract() {
    let root = JobCtx::new();
    let ctx = root.subtask(0.1, 0.5, "Region 1/2");
    apply(&json!({"type":"progress","data":{"prompt_id":"ours","node":"3","value":2,"max":4}}), "ours", &ctx);
    assert!((root.fraction() - 0.31).abs() < 1e-6);
    assert!(root.message().starts_with("Region 1/2"));
    apply(&json!({"type":"execution_cached","data":{"prompt_id":"ours","nodes":["3"]}}), "ours", &ctx);
    assert!((root.fraction() - 0.44).abs() < 1e-6);
}

#[test]
fn websocket_reads_real_events_and_disconnect_falls_back_without_canceling_the_job() {
    use std::net::TcpListener;
    use std::sync::mpsc;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let mut ws = tungstenite::accept(stream).unwrap();
        ws.send(Message::Text(json!({"type":"progress","data":{"prompt_id":"ours","node":"3","value":1,"max":4}}).to_string().into())).unwrap();
        rx.recv_timeout(Duration::from_secs(3)).unwrap();
        ws.send(Message::Text(json!({"type":"progress","data":{"prompt_id":"ours","node":"3","value":3,"max":4}}).to_string().into())).unwrap();
        let _ = ws.close(None);
    });
    let url = endpoint_url(&format!("http://localhost:{port}")).unwrap();
    let mut live = Live::connect(&url, "ours");
    let ctx = JobCtx::new();
    let deadline = Instant::now() + Duration::from_secs(3);
    while ctx.fraction() < 0.3 && Instant::now() < deadline {
        live.poll("ours", &ctx).unwrap();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!((ctx.fraction() - 0.3625).abs() < 1e-6);
    tx.send(()).unwrap();
    server.join().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while ctx.fraction() < 0.6 && Instant::now() < deadline {
        live.poll("ours", &ctx).unwrap();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!((ctx.fraction() - 0.6875).abs() < 1e-6);
    assert!(!ctx.cancelled());
    ctx.cancel();
    assert!(matches!(live.poll("ours", &ctx), Err(EngineError::Cancelled)) || live.socket.is_none());
}
