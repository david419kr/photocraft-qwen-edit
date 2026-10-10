//! Read-only prompt history for the current document, including imported image metadata.
use crate::PhotocraftApp;
use egui::{Context, RichText, vec2};
use photocraft_engine::ai_prompts::Entry;

pub(crate) fn has_history(app: &mut PhotocraftApp, doc: &photocraft_doc::Document, revision: u64) -> bool {
    let key = (doc.id.0, revision);
    if app.ui.ai.prompt_presence_key != Some(key) {
        app.ui.ai.has_prompt_history = photocraft_io::ai_metadata::has_history(doc.metadata.xmp.as_deref());
        app.ui.ai.prompt_presence_key = Some(key);
    }
    app.ui.ai.has_prompt_history
}

pub(crate) fn button(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    let response = crate::icons::button(ui, "info", 24.0, app.ui.ai.prompt_info_open, tl!("Prompt information"));
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), app.ui.ai.prompt_info_open, tl!("Prompt information")));
    if response.clicked() {
        app.ui.ai.prompt_info_open = true;
        app.ui.ai.prompt_info_page = 0;
    }
}

pub(crate) fn only_bar(app: &mut PhotocraftApp, ctx: &Context, document: u64, canvas: egui::Rect, image: egui::Rect) {
    if canvas.width() < 56.0 || canvas.height() < 56.0 {
        return;
    }
    let x = (image.center().x - 20.0).clamp(canvas.left() + 4.0, canvas.right() - 44.0);
    let y = (image.bottom() + 10.0).clamp(canvas.top() + 4.0, canvas.bottom() - 44.0);
    let tokens = crate::theme::Tokens::get(ctx);
    let area = egui::Area::new(egui::Id::new(("ai-prompt-information", document))).order(egui::Order::Foreground).fixed_pos(egui::pos2(x, y)).show(ctx, |ui| {
        egui::Frame::new()
            .fill(tokens.card)
            .stroke(egui::Stroke::new(1.0, tokens.card_border))
            .corner_radius(tokens.radius_sm)
            .inner_margin(8)
            .show(ui, |ui| button(app, ui));
    });
    let rect = area.response.rect;
    app.ui.ai.rect = Some([rect.min.x, rect.min.y, rect.max.x, rect.max.y]);
}

fn text(ui: &mut egui::Ui, value: &str) {
    ui.add(egui::Label::new(value).wrap().selectable(true));
}

fn entry(ui: &mut egui::Ui, number: usize, entry: &Entry) {
    ui.label(RichText::new(format!("{number}. {}", tl!(entry.mode.label()))).strong());
    if !entry.regions.is_empty() {
        let location = if entry.regions.len() == 9 {
            tl!("All nine regions").to_owned()
        } else {
            entry.regions.iter().map(|region| crate::i18n::tr_ctx(crate::i18n::current(), "prompt-region", region.label())).collect::<Vec<_>>().join(" · ")
        };
        ui.label(RichText::new(format!("{location}:")).strong());
    }
    if let Some(translation) = &entry.translated {
        ui.label(tl!("Original prompt"));
        text(ui, &entry.original);
        ui.add_space(4.0);
        ui.label(tl!("English translation"));
        text(ui, translation);
    } else if entry.original.trim().is_empty() {
        ui.label(tl!("No prompt (remove)"));
    } else {
        text(ui, &entry.original);
    }
    ui.add_space(8.0);
    ui.separator();
}

pub(crate) fn show(app: &mut PhotocraftApp, ctx: &Context) {
    if !app.ui.ai.prompt_info_open {
        return;
    }
    let Some(state) = app.session.active() else {
        app.ui.ai.prompt_info_open = false;
        return;
    };
    let doc = state.doc.clone();
    let key = (doc.id.0, state.revision);
    if app.ui.ai.prompt_records_key != Some(key) {
        let same_doc = app.ui.ai.prompt_records_key.is_some_and(|old| old.0 == doc.id.0);
        if !same_doc || app.ui.ai.prompt_records_xmp != doc.metadata.xmp {
            match photocraft_engine::ai_prompts::read(doc.metadata.xmp.as_deref()) {
                Ok(entries) => {
                    app.ui.ai.prompt_records = entries;
                    app.ui.ai.prompt_info_error = None;
                    app.ui.ai.prompt_records_xmp = doc.metadata.xmp.as_ref().filter(|text| text.len() <= photocraft_io::ai_metadata::MAX_XMP_BYTES).cloned();
                }
                Err(error) => {
                    app.ui.ai.prompt_records.clear();
                    app.ui.ai.prompt_info_error = Some(error);
                    app.ui.ai.prompt_records_xmp = None;
                }
            }
        }
        app.ui.ai.prompt_records_key = Some(key);
    }
    let mut open = true;
    let page_size = 50usize;
    let pages = app.ui.ai.prompt_records.len().div_ceil(page_size).max(1);
    let mut page = app.ui.ai.prompt_info_page.min(pages.saturating_sub(1));
    egui::Window::new(tl!("AI Prompt History")).id(egui::Id::new(("ai-prompt-info-window", doc.id.0))).open(&mut open).default_size(vec2(600.0, 420.0)).show(
        ctx,
        |ui| {
            if let Some(error) = &app.ui.ai.prompt_info_error {
                text(ui, error);
                return;
            }
            if app.ui.ai.prompt_records.is_empty() {
                ui.label(tl!("No AI prompts recorded"));
                return;
            }
            if pages > 1 {
                ui.horizontal(|ui| {
                    if ui.add_enabled(page > 0, egui::Button::new(tl!("Previous"))).clicked() {
                        page = page.saturating_sub(1);
                    }
                    ui.label(format!("{} / {pages}", page + 1));
                    if ui.add_enabled(page + 1 < pages, egui::Button::new(tl!("Next"))).clicked() {
                        page = page.saturating_add(1);
                    }
                });
            }
            egui::ScrollArea::vertical().id_salt(("ai-prompt-info-list", doc.id.0, page)).show(ui, |ui| {
                let start = page.saturating_mul(page_size);
                for (index, prompt) in app.ui.ai.prompt_records.iter().enumerate().skip(start).take(page_size) {
                    entry(ui, index + 1, prompt);
                }
            });
        },
    );
    app.ui.ai.prompt_info_open = open;
    app.ui.ai.prompt_info_page = page;
}
