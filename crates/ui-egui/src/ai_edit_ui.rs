//! Existing file picker and command dialog, without exposing model-frame settings.
use crate::{PhotocraftApp, state::DialogKind};
use photocraft_engine::ai_cmds::{self, reference};
use serde_json::{Map, Value, json};
use std::sync::Arc;

const MARK: &str = "__aiResize";
pub fn owns(fields: &Map<String, Value>) -> bool {
    fields.contains_key(MARK)
}

pub fn intercept(app: &mut PhotocraftApp, command: &str, params: &Value) -> Option<Result<Value, String>> {
    if matches!(command, "edit.promptEdit" | reference::SET) && app.session.active().is_some_and(|d| app.session.job_on(d.doc.id).is_some()) {
        return Some(Err("An operation is still running on this document".into()));
    }
    if command == reference::SET && params.get("data").is_none() {
        let doc = app.session.active()?.doc.id;
        return Some(app.pick_file_bytes_filtered(&["png", "jpg", "jpeg", "webp", "avif", "tif", "tiff", "bmp", "psd"], move |app, name, bytes| {
            if bytes.len() > reference::MAX_FILE_BYTES {
                return Err("The reference image file is too large".into());
            }
            if app.session.active().is_none_or(|d| d.doc.id != doc) {
                return Err("The document changed while choosing a reference image".into());
            }
            app.run(reference::SET, json!({"name":name,"data":photocraft_paint::tile::b64_encode(&bytes)}))
        }));
    }
    if command != "edit.promptEdit" || params.get("prompt").is_none() || params.get("resizeApproval").is_some() {
        return None;
    }
    let plan = match ai_cmds::edit_preflight(&app.session) {
        Ok(p) => p,
        Err(e) => return Some(Err(e.to_string())),
    };
    if plan["needsResizeApproval"] != true {
        return None;
    }
    if let Some(existing) = app.ui.dialogs.iter().find(|d| owns(&d.fields)) {
        return Some(Ok(json!({"dialog":existing.id,"confirmationPending":true})));
    }
    let fields = json!({MARK:true,"__label":"Resize for AI editing","plan":plan,"params":params});
    let id = app.ui.open_dialog(DialogKind::Command, fields.as_object().cloned().unwrap_or_default());
    Some(Ok(json!({"dialog":id,"confirmationPending":true})))
}

pub fn body(ui: &mut egui::Ui, fields: &Map<String, Value>) {
    ui.label(tl!("The image exceeds the AI editing size. Resize the document to continue?"));
    let plan = fields.get("plan").unwrap_or(&Value::Null);
    let size = |key: &str| {
        plan.get(key)
            .and_then(Value::as_array)
            .map(|v| format!("{} × {}", v.first().and_then(Value::as_u64).unwrap_or(0), v.get(1).and_then(Value::as_u64).unwrap_or(0)))
            .unwrap_or_default()
    };
    ui.label(format!("{} → {}", size("nativeSize"), size("targetSize")));
}

pub fn confirm(app: &mut PhotocraftApp, fields: &Map<String, Value>) -> Result<Value, String> {
    let mut params = fields.get("params").cloned().ok_or("Missing AI edit request")?;
    params["resizeApproval"] = fields.get("plan").and_then(|p| p.get("approval")).cloned().ok_or("Missing resize approval")?;
    let result = app.run("edit.promptEdit", params);
    if result.is_ok() {
        app.ui.ai.open = false;
    }
    result
}

pub fn cancelled(app: &mut PhotocraftApp, fields: &Map<String, Value>) {
    if owns(fields) && fields.get("plan").and_then(|p| p.get("document")).and_then(Value::as_u64) == app.session.active().map(|d| d.doc.id.0) {
        app.ui.ai.open = true;
    }
}

pub fn reference_controls(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    if crate::icons::button(ui, "image", 24.0, false, tl!("Attach Reference Image")).clicked() {
        let _ = app.run(reference::SET, json!({}));
    }
    let current = reference::current(&app.session);
    app.ui.ai.reference = current.as_ref().map(|r| json!({"id":r.id,"name":r.name,"size":r.content}));
    let Some(reference) = current else {
        app.ui.ai.reference_preview = None;
        return;
    };
    if app.ui.ai.reference_preview.as_ref().is_none_or(|old| !Arc::ptr_eq(&old.0, &reference)) {
        let [width, height] = reference.content.map(|v| v as usize);
        let mut pixels = Vec::with_capacity(width * height * 4);
        for row in reference.image.pixels.chunks_exact(reference.image.width as usize * 4).take(height) {
            let Some(content) = row.get(..width * 4) else {
                return;
            };
            pixels.extend_from_slice(content);
        }
        if pixels.len() != width * height * 4 {
            return;
        }
        let image = egui::ColorImage::from_rgba_unmultiplied([width, height], &pixels);
        let texture = ui.ctx().load_texture(format!("ai-reference-{}", reference.id), image, egui::TextureOptions::LINEAR);
        app.ui.ai.reference_preview = Some(crate::ai_bar::ReferencePreview(reference.clone(), texture));
    }
    if let Some(preview) = &app.ui.ai.reference_preview {
        ui.add(egui::Image::new(&preview.1).fit_to_exact_size(egui::Vec2::splat(24.0))).on_hover_text(&reference.name);
    }
    if crate::icons::button(ui, "x", 20.0, false, tl!("Remove Reference Image")).clicked() {
        let _ = app.run(reference::CLEAR, json!({}));
        app.ui.ai.reference = None;
        app.ui.ai.reference_preview = None;
    }
}

pub fn sync_reference(app: &mut PhotocraftApp) {
    let reference = reference::current(&app.session);
    if app.ui.ai.reference_preview.as_ref().is_some_and(|old| reference.as_ref().is_none_or(|r| !Arc::ptr_eq(&old.0, r))) {
        app.ui.ai.reference_preview = None;
    }
    if app.ui.ai.reference.as_ref().and_then(|r| r.get("id")).and_then(Value::as_u64) != reference.as_ref().map(|r| r.id) {
        app.ui.ai.reference = reference.map(|r| json!({"id":r.id,"name":r.name,"size":r.content}));
    }
}
