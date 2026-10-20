//! DesignCraft on Android, aimed at tablets with a keyboard, a stylus and a mouse.
//!
//! Android starts the app through `DesignCraftActivity`, a `NativeActivity`
//! (packaging/android/java: it keeps the latest intent, colours the system bars and does the
//! updater's HTTP and install), which loads this library and calls [`android_main`]. The UI is the
//! desktop one (`designcraft-ui-egui`); what differs:
//! - Files: every file the UI asks for (Open, Place, Save As, the exports, Package, Relink…) is
//!   chosen in DesignCraft's own file browser over shared storage (`files`), which needs "All
//!   files access" granted once in Settings (`file_access`).
//! - Settings and crash recovery live in the app's data folder; documents left unsaved when
//!   Android stops the app in the background reopen on the next start.
//! - Touch, S Pen, mouse, system bars and the keyboard cover (`tablet`, `touch`); the system
//!   clipboard's text through JNI (`clipboard`); documents from other apps ("Open with",
//!   `incoming`); new builds from the GitHub repository the APK was built from (`updater`).
//! - No printing to a printer and no `--control` channel.
//!
//! The approach (NativeActivity, the patched winit, JNI for what Rust can't reach) follows the
//! PdfCraft, VectorCraft and GridCraft Android ports. Elsewhere the crate is empty, so `cargo
//! build --workspace` on a desktop still works.
#![cfg(target_os = "android")]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod clipboard;
mod file_access;
mod files;
mod incoming;
mod tablet;
mod touch;
mod updater;

use std::path::PathBuf;
use std::sync::OnceLock;

use designcraft_engine::Session;
use designcraft_ui_egui::{DesignApp, Services};
use winit::platform::android::activity::AndroidApp;

/// Shared storage as apps see it (the device's "Internal storage").
pub(crate) const SHARED: &str = "/storage/emulated/0";

/// The app's internal data folder (settings, recovery, received documents).
static APP_DIR: OnceLock<PathBuf> = OnceLock::new();

pub(crate) fn app_dir() -> Option<PathBuf> {
    APP_DIR.get().cloned()
}

/// The settings folder (`<app data>/designcraft`), as `~/.config/designcraft` on Linux.
fn config_dir() -> Option<PathBuf> {
    app_dir().map(|d| d.join("designcraft"))
}

/// The interface settings (`ui.json`) and preferences (`prefs.json`), as on the desktop.
fn load_prefs(app: &mut DesignApp) {
    let Some(dir) = config_dir() else { return };
    if let Ok(bytes) = std::fs::read(dir.join("ui.json"))
        && let Ok(ui) = designcraft_ui_egui::UiState::from_preferences_json(&bytes)
    {
        app.ui = ui;
    }
    if let Ok(bytes) = std::fs::read(dir.join("prefs.json"))
        && let Ok(prefs) = serde_json::from_slice::<designcraft_engine::Prefs>(&bytes)
    {
        app.session.prefs = prefs;
    }
}

/// Write the settings: on leaving the foreground (Android may then stop the app without warning)
/// and on exit.
pub(crate) fn save_prefs(app: &DesignApp) {
    let Some(dir) = config_dir() else { return };
    if let Err(e) = std::fs::create_dir_all(&dir) {
        log::warn!("settings: cannot create {}: {e}", dir.display());
        return;
    }
    if let Ok(b) = serde_json::to_vec_pretty(&app.ui)
        && let Err(e) = write_file(&dir.join("ui.json").to_string_lossy(), &b)
    {
        log::warn!("settings not saved: {e}");
    }
    if let Ok(b) = serde_json::to_vec_pretty(&app.session.prefs)
        && let Err(e) = write_file(&dir.join("prefs.json").to_string_lossy(), &b)
    {
        log::warn!("preferences not saved: {e}");
    }
}

/// Write a whole file atomically: Android stopping the app mid-write leaves the previous one.
fn write_file(path: &str, bytes: &[u8]) -> Result<(), String> {
    designcraft_engine::atomic::write_atomic(std::path::Path::new(path), bytes).map_err(|e| e.to_string())
}

/// The file browser's shortcuts: the device's storage and its usual folders, SD cards and USB
/// drives that are plugged in, and DesignCraft's own folder.
pub(crate) fn places() -> Vec<(String, PathBuf)> {
    let mut out = vec![
        ("Internal storage".to_string(), PathBuf::from(SHARED)),
        ("Documents".to_string(), PathBuf::from(SHARED).join("Documents")),
        ("Download".to_string(), PathBuf::from(SHARED).join("Download")),
        ("Pictures".to_string(), PathBuf::from(SHARED).join("Pictures")),
    ];
    if let Ok(entries) = std::fs::read_dir("/storage") {
        let mut volumes: Vec<PathBuf> = entries
            .flatten()
            .filter(|e| {
                let name = e.file_name();
                let name = name.to_string_lossy();
                name != "emulated" && name != "self" && e.path().is_dir()
            })
            .map(|e| e.path())
            .collect();
        volumes.sort();
        for v in volumes {
            let name = v.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            out.push((format!("SD card or USB drive ({name})"), v));
        }
    }
    if let Some(dir) = app_dir() {
        out.push(("App folder".to_string(), dir));
    }
    out
}

/// What the host gives the app on Android.
fn services(picker: &files::Picker) -> Services {
    let (open, save) = (picker.clone(), picker.clone());
    Services {
        pick_open: Some(Box::new(move |purpose: &str| open.open(purpose))),
        pick_save: Some(Box::new(move |name: &str| save.save(name))),
        read: Some(Box::new(|p: &str| std::fs::read(p).map_err(|e| e.to_string()))),
        write: Some(Box::new(write_file)),
        clipboard_text: Some(Box::new(clipboard::text)),
        ..Default::default()
    }
}

/// Entry point, called by android-activity on its own thread once the activity is created.
#[allow(unsafe_code)] // Exported by name: android-activity looks this symbol up.
#[unsafe(no_mangle)]
fn android_main(android: AndroidApp) {
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("DesignCraft"));
    std::panic::set_hook(Box::new(|info| {
        let trace = std::backtrace::Backtrace::force_capture();
        log::error!("internal error: {info}\n{trace}");
    }));

    if let Some(dir) = android.internal_data_path() {
        let _ = APP_DIR.set(dir.clone());
        let tmp = dir.join("tmp");
        let _ = std::fs::create_dir_all(&tmp);
        // Android sets none of these. HOME is shared storage (the file browser's `~`, and a Fonts
        // folder there for the user's own fonts); settings and crash recovery go in the app's
        // private folder, temporary files where the app may write. Nothing else runs yet: this
        // is the app's only thread besides Java's.
        #[allow(unsafe_code)]
        // SAFETY: called before eframe or the app start any thread that could read the environment.
        unsafe {
            std::env::set_var("HOME", SHARED);
            std::env::set_var("XDG_CONFIG_HOME", &dir);
            std::env::set_var("XDG_DATA_HOME", &dir);
            std::env::set_var("TMPDIR", &tmp);
        }
        log::info!("DesignCraft {} data folder {}", env!("CARGO_PKG_VERSION"), dir.display());
    }
    file_access::set_app(android.clone());
    log::info!("All files access: {}", if file_access::granted() { "granted" } else { "not granted" });
    let shell_android = android.clone();

    let options = eframe::NativeOptions { android_app: Some(android), renderer: eframe::Renderer::Wgpu, ..Default::default() };
    let result = eframe::run_native(
        "DesignCraft",
        options,
        Box::new(|cc| {
            if let Some(state) = &cc.wgpu_render_state {
                let info = state.adapter.get_info();
                log::info!("graphics: wgpu on {} ({:?}, {:?})", info.name, info.device_type, info.backend);
            }
            let picker = files::Picker::default();
            let mut session = Session::new();
            // Crash recovery: reopen what a previous run left unsaved (Android stops apps in the
            // background without warning), then keep it current.
            session.recovery_dir = designcraft_engine::recovery::default_dir().and_then(|root| match designcraft_engine::recovery::run_dir(&root) {
                Ok(dir) => Some(dir),
                Err(e) => {
                    log::warn!("recovery is unavailable: {e}");
                    None
                }
            });
            let recovered = session.execute("file.recovery.open", &serde_json::json!({})).ok();
            let mut app = DesignApp::new(session, services(&picker));
            load_prefs(&mut app);
            if let Some(n) = recovered.as_ref().and_then(|r| r["opened"].as_array()).map(Vec::len).filter(|n| *n > 0) {
                app.status(format!("Recovered {n} unsaved document{} from the last session.", if n == 1 { "" } else { "s" }));
            }
            Ok(Box::new(tablet::TabletApp::new(app, picker, shell_android)))
        }),
    );
    if let Err(e) = result {
        log::error!("DesignCraft stopped: {e}");
    }
}
