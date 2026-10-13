//! Fork update UI; network and executable replacement are native capabilities.
use std::sync::mpsc::Receiver;

use serde_json::{Value, json};

use crate::PhotocraftApp;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub tag: String,
    pub asset_name: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
}

pub enum Action {
    Check,
    Download(Release),
    Restart,
    Cancel,
}

pub enum Event {
    Checked(Result<Option<Release>, String>),
    Prepared,
    Failed(String),
}

pub struct Service {
    pub supported: bool,
    pub action: Box<dyn FnMut(Action) -> Result<(), String>>,
    pub events: Receiver<Event>,
}

#[derive(Default)]
pub struct State {
    started: bool,
    checking: bool,
    manual_check: bool,
    downloading: bool,
    prepared: bool,
    pub available: Option<Release>,
    pub message: Option<String>,
}

pub fn set_enabled(app: &mut PhotocraftApp, enabled: bool) -> Result<Value, String> {
    let result = app.run("prefs.set", json!({"path":"general.automaticUpdateCheck","value":enabled}))?;
    crate::prefs_ui::save_preferences(app)?;
    Ok(result)
}

pub fn inspect(app: &PhotocraftApp) -> Value {
    json!({"automaticCheck":app.session.prefs().general.automatic_update_check,
        "checking":app.updates.checking, "downloading":app.updates.downloading,
        "release":app.updates.available.as_ref().map(|r| &r.tag),
        "message":app.updates.message, "supported":app.services.updates.as_ref().is_some_and(|s| s.supported)})
}

fn send(app: &mut PhotocraftApp, action: Action) -> Result<(), String> {
    let service = app.services.updates.as_mut().ok_or("Updates are not available in this build.")?;
    (service.action)(action)
}

pub fn check(app: &mut PhotocraftApp) -> Result<Value, String> {
    if app.updates.checking || app.updates.downloading {
        return Err("An update operation is already in progress.".into());
    }
    send(app, Action::Cancel)?;
    app.updates.prepared = false;
    app.updates.available = None;
    send(app, Action::Check)?;
    app.updates.checking = true;
    app.updates.manual_check = true;
    app.updates.message = None;
    Ok(Value::Null)
}

fn busy_document(app: &PhotocraftApp) -> bool {
    app.saving() || app.session.documents().iter().any(|d| d.is_dirty())
}

pub fn install(app: &mut PhotocraftApp) -> Result<Value, String> {
    if app.updates.downloading {
        return Err("An update is already downloading.".into());
    }
    if !app.services.updates.as_ref().is_some_and(|s| s.supported) {
        return Err("Executable updates are available only on Windows x64.".into());
    }
    if busy_document(app) {
        return Err("Save or close your documents before updating PhotoCraft.".into());
    }
    let release = app.updates.available.clone().ok_or("No update is available.")?;
    // Persist the shared opt-out before a helper is allowed to close the app.
    crate::prefs_ui::save_preferences(app)?;
    if app.updates.prepared {
        if let Err(error) = send(app, Action::Restart) {
            app.updates.prepared = false;
            return Err(error);
        }
        app.allow_close = true;
    } else {
        send(app, Action::Download(release))?;
        app.updates.downloading = true;
    }
    Ok(Value::Null)
}

pub fn dismiss(app: &mut PhotocraftApp) -> Result<Value, String> {
    if app.updates.downloading {
        return Err("Wait for the download to finish before dismissing it.".into());
    }
    send(app, Action::Cancel)?;
    app.updates.available = None;
    app.updates.prepared = false;
    app.updates.message = None;
    Ok(Value::Null)
}

pub fn poll(app: &mut PhotocraftApp, ctx: &egui::Context) {
    if !app.updates.started {
        app.updates.started = true;
        if app.session.prefs().general.automatic_update_check && app.services.updates.as_ref().is_some_and(|s| s.supported) {
            let _ = check(app);
            app.updates.manual_check = false;
        }
    }
    let events: Vec<_> = app.services.updates.as_ref().map(|s| s.events.try_iter().collect()).unwrap_or_default();
    for event in events {
        match event {
            Event::Checked(result) => {
                app.updates.checking = false;
                match result {
                    Ok(release) => {
                        let up_to_date = release.is_none();
                        app.updates.available = if app.updates.manual_check || app.session.prefs().general.automatic_update_check { release } else { None };
                        app.updates.message = up_to_date.then(|| tl!("PhotoCraft is up to date.").to_string());
                    }
                    Err(error) => app.updates.message = Some(error),
                }
            }
            Event::Prepared => {
                app.updates.downloading = false;
                app.updates.prepared = true;
                match install(app) {
                    Ok(_) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                    Err(error) => app.updates.message = Some(error),
                }
            }
            Event::Failed(error) => {
                app.updates.downloading = false;
                app.updates.message = Some(error);
            }
        }
    }
    if app.updates.checking || app.updates.downloading {
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

pub fn show(app: &mut PhotocraftApp, ctx: &egui::Context) {
    let Some(release) = app.updates.available.as_ref() else { return };
    let tag = release.tag.clone();
    egui::Modal::new(egui::Id::new("fork-update")).show(ctx, |ui| {
        ui.set_max_width(420.0);
        ui.heading(tl!("PhotoCraft update available"));
        ui.label(format!("{} → {tag}", photocraft_engine::build_info::release_tag()));
        ui.label(tl!("Update PhotoCraft and restart now?"));
        let mut hidden = !app.session.prefs().general.automatic_update_check;
        if ui.checkbox(&mut hidden, tl!("Don't show again")).changed()
            && let Err(error) = set_enabled(app, !hidden)
        {
            app.updates.message = Some(error);
        }
        if hidden {
            ui.label(tl!("You can check for updates again in Help → About PhotoCraft."));
        }
        if let Some(message) = &app.updates.message {
            ui.label(message);
        }
        if app.updates.downloading {
            ui.spinner();
            ui.label(tl!("Downloading update…"));
        } else {
            ui.horizontal(|ui| {
                if ui.button(tl!("Yes")).clicked() {
                    match install(app) {
                        Ok(_) if app.allow_close => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                        Ok(_) => {}
                        Err(error) => app.updates.message = Some(error),
                    }
                }
                if ui.button(tl!("No")).clicked()
                    && let Err(error) = dismiss(app)
                {
                    app.updates.message = Some(error);
                }
            });
        }
    });
}

pub fn about(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    let mut enabled = app.session.prefs().general.automatic_update_check;
    if ui.checkbox(&mut enabled, tl!("Automatically check for updates")).changed()
        && let Err(error) = set_enabled(app, enabled)
    {
        app.updates.message = Some(error);
    }
    if app.services.updates.is_some()
        && ui.add_enabled(!app.updates.checking, egui::Button::new(tl!("Check for updates now"))).clicked()
        && let Err(error) = check(app)
    {
        app.updates.message = Some(error);
    }
    if let Some(message) = &app.updates.message {
        ui.label(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex, mpsc};

    #[test]
    fn startup_check_respects_the_one_persisted_preference() {
        for enabled in [true, false] {
            let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
            set_enabled(&mut app, enabled).unwrap();
            let calls = Arc::new(Mutex::new(0));
            let count = calls.clone();
            let (_, rx) = mpsc::channel();
            app.services.updates = Some(Service {
                supported: true,
                events: rx,
                action: Box::new(move |action| {
                    if matches!(action, Action::Check) {
                        *count.lock().unwrap() += 1;
                    }
                    Ok(())
                }),
            });
            poll(&mut app, &egui::Context::default());
            poll(&mut app, &egui::Context::default());
            assert_eq!(*calls.lock().unwrap(), usize::from(enabled));
        }
    }

    #[test]
    fn update_requires_a_release_and_preserves_unsaved_work() {
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        let (_, rx) = mpsc::channel();
        app.services.updates = Some(Service { supported: true, events: rx, action: Box::new(|_| panic!("must not install")) });
        assert!(install(&mut app).is_err());
        app.updates.available = Some(Release { tag: "261014-0.8.2".into(), asset_name: String::new(), url: String::new(), size: 1, sha256: String::new() });
        app.run("file.new", json!({"width":2,"height":2})).unwrap();
        app.run("layer.new.layer", json!({})).unwrap();
        assert!(install(&mut app).unwrap_err().contains("Save or close"));
        assert!(!app.allow_close);
    }

    #[test]
    fn manual_check_can_offer_and_install_when_automatic_checks_are_off() {
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        set_enabled(&mut app, false).unwrap();
        let (tx, rx) = mpsc::channel();
        let actions = Arc::new(Mutex::new(Vec::new()));
        let observed = actions.clone();
        app.services.updates = Some(Service {
            supported: true,
            events: rx,
            action: Box::new(move |a| {
                observed.lock().unwrap().push(match a {
                    Action::Check => "check",
                    Action::Download(_) => "download",
                    Action::Restart => "restart",
                    Action::Cancel => "cancel",
                });
                Ok(())
            }),
        });
        let ctx = egui::Context::default();
        poll(&mut app, &ctx);
        check(&mut app).unwrap();
        tx.send(Event::Checked(Ok(Some(Release {
            tag: "261014-0.8.2".into(),
            asset_name: String::new(),
            url: String::new(),
            size: 1,
            sha256: String::new(),
        }))))
        .unwrap();
        poll(&mut app, &ctx);
        assert!(app.updates.available.is_some());
        install(&mut app).unwrap();
        tx.send(Event::Prepared).unwrap();
        poll(&mut app, &ctx);
        assert!(app.allow_close);
        assert_eq!(*actions.lock().unwrap(), ["cancel", "check", "download", "restart"]);
        assert!(!app.session.prefs().general.automatic_update_check);
    }

    #[test]
    fn popup_opt_out_and_about_checkbox_share_a_single_preference() {
        use egui_kittest::{Harness, kittest::Queryable};
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        app.updates.available = Some(Release { tag: "261014-0.8.2".into(), asset_name: String::new(), url: String::new(), size: 1, sha256: String::new() });
        let mut h = Harness::builder().build_ui_state(|ui, app| show(app, ui.ctx()), app);
        h.run_steps(2);
        h.get_by_label("Don't show again").click();
        h.run_steps(2);
        assert!(!h.state().session.prefs().general.automatic_update_check);
        assert!(h.query_by_label("You can check for updates again in Help → About PhotoCraft.").is_some());
        h.state_mut().updates.available = None;
        let app = std::mem::replace(h.state_mut(), PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default()));
        let mut h = Harness::builder().build_ui_state(|ui, app| about(app, ui), app);
        h.run_steps(2);
        h.get_by_label("Automatically check for updates").click();
        h.run_steps(2);
        assert!(h.state().session.prefs().general.automatic_update_check);
    }
}
