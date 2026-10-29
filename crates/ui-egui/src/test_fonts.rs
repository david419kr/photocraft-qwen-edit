//! Real, pinned Korean glyphs for headless fork UI tests, including builds without craft-fonts.
//! Test-only fixture; source, hash and OFL attribution are recorded in ATTRIBUTION.md.

use egui::FallbackRequest;
use egui::epaint::text::FontInsert;
use std::sync::Arc;

pub(crate) fn korean(ctx: &egui::Context) {
    ctx.add_font_provider(Arc::new(|request: &FallbackRequest<'_>| {
        let c = request.cluster.chars().next()?;
        if !('가'..='힣').contains(&c) {
            return None;
        }
        Some(FontInsert {
            name: "fork-test-NanumGothic".into(),
            data: egui::FontData::from_static(include_bytes!("../tests/fonts/NanumGothic-Regular.ttf")),
            families: Vec::new(),
        })
    }));
}
