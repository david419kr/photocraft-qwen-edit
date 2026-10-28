//! All passes use frozen native source data; no document commit until every pass succeeds.
use super::{AiBackend, AiRequest, Arc, Document, EditMode, EditRegion, EngineError, JobCtx, Layer, Result, fill, harmonize, result_layer};

#[derive(Clone)]
pub(super) struct Pass {
    pub request: AiRequest,
    pub region: EditRegion,
}

pub(super) fn seed(root: u64, index: usize) -> u64 {
    // A fixed, bijective mixing of the group number gives reproducible independent streams.
    let mut value = (index as u64).wrapping_mul(0x9e3779b97f4a7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    root ^ value ^ (value >> 31)
}

pub(super) fn generate(
    source: Arc<Document>,
    backend: &dyn AiBackend,
    request: &AiRequest,
    region: &EditRegion,
    extra: &[Pass],
    name: &str,
    ctx: &JobCtx,
) -> Result<(Layer, Arc<harmonize::Source>)> {
    if extra.len() >= 4 {
        return Err(EngineError::Other("too many fill groups".into()));
    }
    let mut layers = Vec::new();
    let mut records = Vec::new();
    for (index, (request, region)) in std::iter::once((request, region)).chain(extra.iter().map(|p| (&p.request, &p.region))).enumerate() {
        ctx.check()?;
        let count = (extra.len() + 1) as f32;
        let pass_ctx = ctx.subtask(
            0.08 + 0.86 * index as f32 / count,
            0.08 + 0.86 * (index + 1) as f32 / count,
            &format!("Editing region {}/{}", index + 1, extra.len() + 1),
        );
        pass_ctx.progress(0.0, "Preparing request");
        let generated = backend.generate(request, &pass_ctx)?;
        layers.push(result_layer(&source, &generated, region, name, EditMode::GenerativeFill, &pass_ctx)?);
        records.push(harmonize::Pass { original: request.image.clone(), generated, region: region.clone() });
    }
    let layer =
        if layers.len() == 1 { layers.pop().ok_or_else(|| EngineError::Other("empty fill result".into()))? } else { fill::merge(&source, layers, name, ctx)? };
    let record = Arc::new(harmonize::Source::new_many(source, records, layer.clone()));
    Ok((layer, record))
}
