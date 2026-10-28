//! cargo run --release -p photocraft-engine --example ai_mask_benchmark
//! 24MP preparation/restoration/assembly only; this never contacts an AI service.
use photocraft_engine::{
    Result, Session,
    ai_cmds::{AiBackend, AiRequest},
    jobs::JobCtx,
};
use photocraft_raster::Rgba8Image;
use serde_json::json;
use std::{sync::Arc, time::Instant};

struct Fake;
impl AiBackend for Fake {
    fn generate(&self, request: &AiRequest, ctx: &JobCtx) -> Result<Rgba8Image> {
        ctx.check()?;
        let mut output = request.image.clone();
        for pixel in output.pixels.as_chunks_mut::<4>().0 {
            pixel[3] = 192;
        }
        Ok(output)
    }
}

fn main() -> Result<()> {
    println!("PERF_START:{}", std::process::id());
    for experimental in [false, true] {
        let mut session = Session::new();
        session.ai_backend = Some(Arc::new(Fake));
        session.execute("file.new", json!({"width":6000,"height":4000,"background":"#b4a08c"}))?;
        let begin = Instant::now();
        let result = session.execute("edit.aiRemoveBackground", json!({"steps":4,"experimentalRefine":experimental}))?;
        println!(
            "PERF_RESULT:{}",
            json!({"size":[6000,4000],"seconds":begin.elapsed().as_secs_f64(),"experimentalRefine":experimental,"inference":"fake","geometry":result})
        );
    }
    Ok(())
}
