//! Synchronized 24-MP mixed-mode batch timing (or `--quick` for 1024x768).
//! `cargo run --release -p photocraft-gpu --example bench_mixed_batch`
use std::time::{Duration, Instant};

use photocraft_color::{BlendMode, Color, ColorMode, SampleType};
use photocraft_doc::{Document, Fill, Layer, LayerContent};
use photocraft_geom::Size;
use photocraft_gpu::{Compositor, Stats};

fn render(comp: &mut Compositor, device: &wgpu::Device, queue: &wgpu::Queue, doc: &Document) -> Result<Stats, Box<dyn std::error::Error>> {
    let stats = comp.render(device, queue, doc, doc.bounds(), |_, _| {})?;
    device.poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(Duration::from_secs(60)) })?;
    if stats.layers_batched < 2 || stats.normal_batches != 0 {
        return Err(format!("fixture did not exercise a general layer batch: {stats:?}").into());
    }
    Ok(stats)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let size = if std::env::args().any(|arg| arg == "--quick") { Size::new(1024, 768) } else { Size::new(6000, 4000) };
    let adapter = pollster::block_on(wgpu::Instance::default().request_adapter(&Default::default()))?;
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default()))?;
    let mut doc = Document::new("mixed batch", size, ColorMode::Rgb, SampleType::U8);
    for (mode, rgb) in [(BlendMode::Normal, [0.2, 0.4, 0.6]), (BlendMode::Multiply, [0.7, 0.5, 0.3]), (BlendMode::Difference, [0.1, 0.3, 0.8])] {
        let color = Color { mode: ColorMode::Rgb, c: [rgb[0], rgb[1], rgb[2], 0.0], alpha: 0.8 };
        let mut layer = Layer::new("mixed", LayerContent::Fill(Fill::Solid(color)));
        layer.blend = mode;
        doc.layers.push(layer);
    }
    let mut comp = Compositor::try_new_with_format(&device, Compositor::preferred_acc_format(&adapter))?;
    comp.set_memory_budget(256 << 20);
    comp.set_texture_limit(1024);
    comp.set_composite_cache_budget(0);
    println!("adapter={:?}, size={}x{}", adapter.get_info(), size.width, size.height);
    render(&mut comp, &device, &queue, &doc)?;
    for opacity in [0.7, 0.4, 0.8] {
        doc.layers.last_mut().ok_or("missing layer")?.opacity = opacity;
        let start = Instant::now();
        let stats = render(&mut comp, &device, &queue, &doc)?;
        println!(
            "opacity={opacity}, ms={:.3}, layers_batched={}, normal_batches={}",
            start.elapsed().as_secs_f64() * 1000.0,
            stats.layers_batched,
            stats.normal_batches
        );
    }
    Ok(())
}
