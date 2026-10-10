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

fn prompt_text(ui: &mut egui::Ui, number: usize, field: &str, value: &str, copy_label: &str) {
    ui.horizontal_top(|ui| {
        let button_size = 24.0;
        let width = (ui.available_width() - button_size - ui.spacing().item_spacing.x).max(1.0);
        let id = ui.make_persistent_id(("ai-prompt-text", number, field));
        let menu_selection = id.with("copy-selection");
        let previous_range = egui::text_edit::TextEditState::load(ui.ctx(), id).and_then(|state| state.cursor.char_range());
        // An immutable TextBuffer still gets text focus, so Copy is not routed to canvas pixels.
        let mut read_only = value;
        let output = egui::TextEdit::multiline(&mut read_only)
            .id(id)
            .font(egui::TextStyle::Body)
            .frame(egui::Frame::NONE)
            .margin(0)
            .desired_rows(1)
            .desired_width(width)
            .show(ui);
        // egui collapses a text selection on a secondary mouse press and when the menu takes focus.
        if (output.response.contains_pointer() && ui.input(|input| input.pointer.button_pressed(egui::PointerButton::Secondary)))
            || output.response.secondary_clicked()
        {
            let selected = previous_range.filter(|range| !range.is_empty()).map(|range| range.slice_str(value)).unwrap_or(value);
            ui.ctx().data_mut(|data| data.insert_temp(menu_selection, selected.to_owned()));
            let mut state = output.state.clone();
            state.cursor.set_char_range(previous_range);
            state.store(ui.ctx(), id);
        }
        output.response.context_menu(|ui| {
            if ui.button(tl!("Copy")).clicked() {
                let selected = ui.ctx().data_mut(|data| data.remove_temp::<String>(menu_selection)).unwrap_or_else(|| value.to_owned());
                ui.ctx().copy_text(selected);
                ui.close();
            }
        });
        let copy = crate::icons::button(ui, "copy", button_size, false, copy_label);
        copy.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), false, copy_label));
        if copy.clicked() {
            ui.ctx().copy_text(value.to_owned());
        }
    });
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
        prompt_text(ui, number, "original", &entry.original, tl!("Copy original prompt"));
        ui.add_space(4.0);
        ui.label(tl!("English translation"));
        prompt_text(ui, number, "translation", translation, tl!("Copy English translation"));
    } else if entry.original.trim().is_empty() {
        ui.label(tl!("No prompt (remove)"));
    } else {
        prompt_text(ui, number, "original", &entry.original, tl!("Copy original prompt"));
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

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Key, Modifiers, PointerButton};
    use egui_kittest::{Harness, kittest::Queryable};
    use photocraft_engine::{Session, ai_cmds::EditMode};
    use serde_json::json;

    const ORIGINAL: &str = "원문 한글 🌙\n두 번째 줄";
    const ENGLISH: &str = "A girl on a hill\nDraw a moon.";

    fn harness() -> Harness<'static, PhotocraftApp> {
        let mut session = Session::new();
        session.execute("file.new", json!({"width":64,"height":64})).unwrap();
        session.execute("prefs.set", json!({"path":"interface.language","value":"en"})).unwrap();
        let record = Entry { mode: EditMode::PromptEdit, regions: Vec::new(), original: ORIGINAL.into(), translated: Some(ENGLISH.into()) };
        session
            .edit("Prompt fixture", |doc, _| {
                doc.metadata.xmp = Some(photocraft_engine::ai_prompts::append(None, &record).unwrap());
                Ok(())
            })
            .unwrap();
        let mut h = Harness::builder().with_size(vec2(1200.0, 900.0)).with_max_steps(64).build_eframe(move |cc| {
            PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
            let mut app = PhotocraftApp::new(session, crate::Services::default());
            app.ui.ai.prompt_info_open = true;
            app
        });
        h.run_steps(6);
        h
    }

    fn copied(h: &Harness<'_, PhotocraftApp>) -> Vec<String> {
        h.output()
            .platform_output
            .commands
            .iter()
            .filter_map(|command| match command {
                egui::OutputCommand::CopyText(value) => Some(value.clone()),
                _ => None,
            })
            .collect()
    }

    fn focus_original(h: &mut Harness<'_, PhotocraftApp>) {
        let center = h.get_by_value(ORIGINAL).rect().center();
        h.hover_at(center);
        h.drag_at(center);
        h.drop_at(center);
        h.step();
        assert!(h.ctx.text_edit_focused());
    }

    #[test]
    fn prompt_copy_buttons_copy_only_each_complete_prompt() {
        let mut h = harness();
        h.get_by_label("Copy original prompt").click();
        h.step();
        assert_eq!(copied(&h), vec![ORIGINAL]);
        h.get_by_label("Copy English translation").click();
        h.step();
        assert_eq!(copied(&h), vec![ENGLISH]);
        assert!(h.state().session.clipboard.is_none());
    }

    #[test]
    fn prompt_copy_keeps_text_focus_and_never_edits_saved_history() {
        let mut h = harness();
        let xmp = h.state().session.active().unwrap().doc.metadata.xmp.clone();
        focus_original(&mut h);
        h.key_press_modifiers(Modifiers::COMMAND, Key::A);
        h.step();
        // Native Ctrl+C arrives as Copy; the app's real raw_input_hook must preserve it.
        let ctx = h.ctx.clone();
        let mut raw = egui::RawInput { events: vec![Event::Copy], ..Default::default() };
        eframe::App::raw_input_hook(h.state_mut(), &ctx, &mut raw);
        assert_eq!(raw.events, vec![Event::Copy]);
        for event in raw.events {
            h.event(event);
        }
        h.step();
        assert_eq!(copied(&h), vec![ORIGINAL]);
        assert!(h.state().session.clipboard.is_none());
        h.event(Event::Paste("replace".into()));
        h.event(Event::Text("typing".into()));
        h.key_press(Key::Delete);
        h.step();
        assert_eq!(h.state().session.active().unwrap().doc.metadata.xmp, xmp);
        assert!(h.query_by_value(ORIGINAL).is_some());
    }

    #[test]
    fn prompt_context_menu_copies_unicode_selection() {
        let mut h = harness();
        focus_original(&mut h);
        let id = h.ctx.memory(|memory| memory.focused()).unwrap();
        let mut state = egui::text_edit::TextEditState::load(&h.ctx, id).unwrap();
        state.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(2))));
        state.store(&h.ctx, id);
        let center = h.get_by_value(ORIGINAL).rect().center();
        h.hover_at(center);
        for pressed in [true, false] {
            h.event(Event::PointerButton { pos: center, button: PointerButton::Secondary, pressed, modifiers: Modifiers::NONE });
        }
        h.step();
        h.get_by_label("Copy").click();
        h.step();
        assert_eq!(copied(&h), vec!["원문"]);
        assert!(h.state().session.clipboard.is_none());
    }
}
