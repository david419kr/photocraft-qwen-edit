//! Session-local, immutable reference images. Files never enter the engine as ambient paths.
use super::{Arc, CommandSpec, EngineError, Result, Rgba8Image, Session, Value, bad, can_edit, json, prepare_image, resolution, supported_document};
use std::sync::atomic::{AtomicU64, Ordering};

pub const SET: &str = "edit.aiReference.set";
pub const CLEAR: &str = "edit.aiReference.clear";
pub const INFO: &str = "edit.aiReference.info";
pub const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub struct Reference {
    pub id: u64,
    pub name: String,
    pub image: Arc<Rgba8Image>,
    pub content: [u32; 2],
}

pub fn current(s: &Session) -> Option<Arc<Reference>> {
    s.active().and_then(|d| s.ai_references.get(&d.doc.id)).cloned()
}

fn info(s: &Session) -> Value {
    current(s).map_or(Value::Null, |r| json!({"id":r.id,"name":r.name,"contentSize":r.content,"workingSize":[r.image.width,r.image.height]}))
}

fn set(s: &mut Session, p: &Value) -> Result<Value> {
    let name = p.get("name").and_then(Value::as_str).filter(|s| !s.is_empty() && s.len() <= 512).ok_or_else(|| bad(SET, "name must contain 1..512 bytes"))?;
    let name = name.rsplit(['/', '\\']).next().filter(|s| !s.is_empty()).ok_or_else(|| bad(SET, "invalid name"))?.to_owned();
    let data = p
        .get("data")
        .and_then(Value::as_str)
        .filter(|s| s.len() <= MAX_FILE_BYTES / 3 * 4 + 4)
        .ok_or_else(|| bad(SET, "data must be bounded base64 file bytes"))?;
    let bytes = photocraft_paint::tile::b64_decode(data).filter(|b| b.len() <= MAX_FILE_BYTES).ok_or_else(|| bad(SET, "invalid reference data"))?;
    crate::jobs::run(
        s,
        "Preparing reference image",
        true,
        move |ctx| {
            ctx.check()?;
            let doc = ctx
                .stage(0.0, 0.6, "Reading reference image", |ctl| photocraft_io::import_with(&name, &bytes, ctl))
                .map_err(|e| EngineError::Other(e.to_string()))?
                .document;
            supported_document(&doc).map_err(EngineError::Other)?;
            let frame =
                resolution::Budget::ATTACHED_REFERENCE.plan([doc.size.width, doc.size.height], 0).ok_or_else(|| bad(SET, "invalid reference dimensions"))?;
            let input = prepare_image(&doc, frame.content, ctx)?;
            Ok(Arc::new(Reference { id: NEXT_ID.fetch_add(1, Ordering::Relaxed), name, image: Arc::new(input.image), content: frame.content }))
        },
        |s, r| {
            let id = s.active().ok_or(EngineError::NoDocument)?.doc.id;
            s.ai_references.insert(id, r);
            Ok(info(s))
        },
    )
}

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec {
            id: SET,
            label: "Attach Reference Image",
            menu: &[],
            shortcut: None,
            params: r#"{"name":file name,"data":base64 file bytes} → {id,name,contentSize,workingSize}; session-only, no document history"#,
            enabled: can_edit,
            run: set,
            journal: false,
        },
        CommandSpec {
            id: CLEAR,
            label: "Remove Reference Image",
            menu: &[],
            shortcut: None,
            params: "{} → null",
            enabled: can_edit,
            run: |s, p| {
                if !p.is_object() {
                    return Err(bad(CLEAR, "use an object"));
                }
                let id = s.active().ok_or(EngineError::NoDocument)?.doc.id;
                s.ai_references.remove(&id);
                Ok(Value::Null)
            },
            journal: false,
        },
        CommandSpec {
            id: INFO,
            label: "Reference Image",
            menu: &[],
            shortcut: None,
            params: "{} → reference metadata|null (no image bytes)",
            enabled: can_edit,
            run: |s, p| {
                if !p.is_object() {
                    return Err(bad(INFO, "use an object"));
                }
                Ok(info(s))
            },
            journal: false,
        },
    ]
}
