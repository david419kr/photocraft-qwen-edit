//! CPU-only AI bridge timing; the echo backend does not contact ComfyUI or use a GPU.
use std::sync::Arc;
use std::time::Instant;

use photocraft_engine::ai_cmds::{AiBackend, AiRequest};
use photocraft_engine::jobs::JobCtx;
use photocraft_engine::{Result, Session};
use photocraft_raster::Rgba8Image;
use serde_json::json;

struct Echo;
impl AiBackend for Echo {
    fn generate(&self, request: &AiRequest, ctx: &JobCtx) -> Result<Rgba8Image> {
        ctx.check()?;
        let mut image = request.image.clone();
        for pixel in image.pixels.as_chunks_mut::<4>().0.iter_mut() {
            if request.mode == photocraft_engine::ai_cmds::EditMode::GenerativeFill {
                for value in pixel.iter_mut().take(3) {
                    *value = value.saturating_sub(16);
                }
            }
            if let Some(alpha) = pixel.get_mut(3) {
                *alpha = 255;
            }
        }
        Ok(image)
    }
}

fn main() -> Result<()> {
    let mut session = Session::new();
    session.execute("file.new", json!({"width":6000,"height":4000,"background":"white"}))?;
    session.ai_backend = Some(Arc::new(Echo));
    let fill = std::env::args().any(|arg| arg == "--fill");
    if fill {
        session.execute("select.rect", json!({"x":1500,"y":1000,"width":3000,"height":2000}))?;
    }
    let command = if std::env::args().any(|arg| arg == "--cutout") {
        photocraft_engine::ai_cmds::REMOVE_BACKGROUND_COMMAND
    } else if fill {
        "edit.generativeFill"
    } else {
        "edit.promptEdit"
    };
    let started = Instant::now();
    let result = session.execute(command, json!({"prompt":"benchmark","seed":1}))?;
    println!("{}", json!({"documentMP":24,"command":command,"cpuBridgeMs":started.elapsed().as_millis(),"result":result,"realGeneration":false}));
    if fill && std::env::args().any(|arg| arg == "--poisson") {
        for strength in [100, 35, 0, 35] {
            let started = Instant::now();
            session.execute(photocraft_engine::ai_cmds::harmonize::COMMAND, json!({"strength":strength}))?;
            println!("{}", json!({"documentMP":24,"postprocess":"poisson","strength":strength,"cpuMs":started.elapsed().as_millis(),"realGeneration":false}));
        }
    }
    Ok(())
}
