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
        for pixel in image.pixels.chunks_exact_mut(4) {
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
    let started = Instant::now();
    let result = session.execute("edit.promptEdit", json!({"prompt":"benchmark","seed":1}))?;
    println!("{}", json!({"documentMP":24,"cpuBridgeMs":started.elapsed().as_millis(),"result":result,"realGeneration":false}));
    Ok(())
}
