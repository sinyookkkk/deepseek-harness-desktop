//! Tauri application wiring: menus, window, Import Runtime, restart, and shutdown.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, RwLock};
use std::thread;
use std::time::Duration;

use semver::Version;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Manager, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::config::{self, Config};
use crate::logging::SidecarLog;
use crate::paths::{self, Layout};
use crate::runtime::{self, Pointer, RuntimeSource};
use crate::sidecar::{self, Session};

/// Shared application state.
pub struct AppState {
    /// Loaded configuration.
    pub config: Config,
    /// Resolved paths.
    pub layout: Layout,
    /// Sidecar stderr log.
    pub log: SidecarLog,
    /// Persisted runtime selection.
    pub pointer: RwLock<Option<Pointer>>,
    /// Runtime selected for this launch.
    pub runtime: RwLock<SelectedRuntime>,
    /// Running sidecar session.
    pub session: Mutex<Option<Session>>,
    /// Origin published by the current sidecar, allowed for window navigation.
    pub origin: Mutex<Option<String>>,
    /// Monotonic launch identifier.
    generation: AtomicU64,
}

/// The runtime this shell starts.
#[derive(Debug, Clone)]
pub struct SelectedRuntime {
    /// Executable path.
    pub exe: PathBuf,
    /// Reported semantic version.
    pub version: Version,
    /// Selection origin.
    pub source: RuntimeSource,
}

impl AppState {
    /// @returns a new launch identifier.
    pub fn next_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// @param generation - launch identifier to test.
    /// @returns whether `generation` is still the newest launch.
    pub fn is_latest_launch(&self, generation: u64) -> bool {
        self.generation.load(Ordering::SeqCst) == generation
    }

    /// Remove the session belonging to one launch, leaving newer sessions alone.
    ///
    /// @param generation - launch identifier of the supervisor asking.
    pub fn take_session(&self, generation: u64) -> Option<Session> {
        let mut slot = self.session.lock().ok()?;
        if slot
            .as_ref()
            .is_some_and(|session| session.generation == generation)
        {
            slot.take()
        } else {
            None
        }
    }

    /// Terminate and forget the current sidecar tree.
    pub fn terminate_session(&self) {
        if let Ok(mut slot) = self.session.lock() {
            if let Some(session) = slot.take() {
                session.terminate();
            }
        }
    }

    /// Restore the retained runtime after an imported runtime fails its first start.
    ///
    /// Only the newest launch may roll back, and only while the pointer still
    /// names the runtime that launch selected. Without a retained version the
    /// pointer is removed so the next start uses the bundled seed.
    ///
    /// @param generation - launch that selected the failing runtime.
    /// @param selected_version - version that launch selected.
    pub fn restore_after_failed_start(&self, generation: u64, selected_version: &str) {
        if !self.is_latest_launch(generation) {
            return;
        }
        let Ok(mut slot) = self.pointer.write() else {
            return;
        };
        let Some(current) = slot.as_ref() else {
            return;
        };
        if current.source != "import" || current.version != selected_version {
            return;
        }
        let pointer_file = self.layout.pointer_file();
        match current.previous.clone() {
            Some(previous) => {
                let restored = Pointer {
                    version: previous,
                    source: "restored".to_owned(),
                    previous: None,
                };
                if let Err(error) = restored.save(&pointer_file) {
                    self.log
                        .append(&format!("could not restore the previous runtime: {error}"));
                    return;
                }
                *slot = Some(restored);
            }
            None => {
                if let Err(error) = std::fs::remove_file(&pointer_file) {
                    if error.kind() != std::io::ErrorKind::NotFound {
                        self.log.append(&format!(
                            "could not remove {}: {error}",
                            pointer_file.display()
                        ));
                        return;
                    }
                }
                *slot = None;
            }
        }
        drop(slot);
        self.reload_selected_runtime();
    }

    /// Mark an imported runtime as proven after its first successful start.
    ///
    /// A later startup failure then reports the error without downgrading.
    ///
    /// @param generation - launch that started the runtime.
    /// @param selected_version - version that launch selected.
    pub fn mark_runtime_stable(&self, generation: u64, selected_version: &str) {
        if !self.is_latest_launch(generation) {
            return;
        }
        let Ok(mut slot) = self.pointer.write() else {
            return;
        };
        let Some(current) = slot.as_ref() else {
            return;
        };
        if current.source != "import" || current.version != selected_version {
            return;
        }
        let stable = Pointer {
            version: current.version.clone(),
            source: "installed".to_owned(),
            previous: None,
        };
        if stable.save(&self.layout.pointer_file()).is_ok() {
            *slot = Some(stable);
        }
    }

    /// Rebuild the selected runtime from the pointer, falling back to the seed.
    pub fn reload_selected_runtime(&self) {
        let selected = {
            let pointer = self.pointer.read().ok();
            pointer.as_ref().and_then(|pointer| {
                pointer
                    .as_ref()
                    .and_then(|pointer| self.selected_from_pointer(pointer))
            })
        }
        .or_else(|| self.selected_from_seed());
        if let Some(selected) = selected {
            if let Ok(mut runtime) = self.runtime.write() {
                *runtime = selected;
            }
        }
    }

    fn selected_from_pointer(&self, pointer: &Pointer) -> Option<SelectedRuntime> {
        let exe = self
            .layout
            .runtimes_dir()
            .join(&pointer.version)
            .join("dsh.exe");
        if !exe.is_file() {
            return None;
        }
        let version = Version::parse(&pointer.version).ok()?;
        Some(SelectedRuntime {
            exe,
            version,
            source: RuntimeSource::Imported,
        })
    }

    fn selected_from_seed(&self) -> Option<SelectedRuntime> {
        let exe = self.layout.seed_runtime();
        let version = runtime::probe_version(&exe).ok()?;
        Some(SelectedRuntime {
            exe,
            version,
            source: RuntimeSource::Seed,
        })
    }
}

/// Run the desktop shell.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            if let Err(message) = boot(app.handle()) {
                eprintln!("dsh desktop: {message}");
                show_error_and_exit(app.handle(), &message);
            }
            Ok(())
        })
        .on_menu_event(handle_menu_event)
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { .. } = event {
                window.state::<AppState>().terminate_session();
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run the desktop shell");
}

fn boot(app: &AppHandle) -> Result<(), String> {
    let local_data = app
        .path()
        .app_local_data_dir()
        .map_err(|error| format!("Cannot resolve the local application data directory: {error}"))?;
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|error| format!("Cannot resolve the resource directory: {error}"))?;
    let resources = paths::resources_root(
        cfg!(debug_assertions),
        Path::new(env!("CARGO_MANIFEST_DIR")),
        &resource_dir,
    );
    let layout = Layout::new(local_data, resources);
    let loaded = config::load_or_create(&layout.config_file(), &config::default_workspace())?;
    if loaded.created {
        eprintln!(
            "dsh desktop: created configuration at {}",
            layout.config_file().display()
        );
    }
    let log = SidecarLog::new(layout.log_file())?;
    let pointer_file = layout.pointer_file();
    let (mut pointer, pointer_notice) = Pointer::load_or_fallback(&pointer_file);
    if let Some(notice) = &pointer_notice {
        let corrupt = pointer_file.with_extension("json.corrupt");
        let _ = std::fs::rename(&pointer_file, corrupt);
        log.append(notice);
    }
    let (selection, selection_notice) = runtime::select(
        pointer.as_ref(),
        &layout.runtimes_dir(),
        &layout.seed_runtime(),
    )?;
    if selection.source == RuntimeSource::Seed && pointer.is_some() {
        // The recorded runtime is unavailable; keep the fallback one-time.
        if let Err(error) = std::fs::remove_file(&pointer_file) {
            if error.kind() != std::io::ErrorKind::NotFound {
                log.append(&format!(
                    "could not remove {}: {error}",
                    pointer_file.display()
                ));
            }
        }
        pointer = None;
        log.append("removed the unavailable runtime record; using the bundled runtime");
    }
    let version = match selection.version {
        Some(version) => version,
        None => runtime::probe_version(&selection.exe)?,
    };
    let selected = SelectedRuntime {
        exe: selection.exe,
        version,
        source: selection.source,
    };
    let mut notices = Vec::new();
    notices.extend(pointer_notice);
    notices.extend(selection_notice);

    app.manage(AppState {
        config: loaded.config,
        layout,
        log,
        pointer: RwLock::new(pointer),
        runtime: RwLock::new(selected),
        session: Mutex::new(None),
        origin: Mutex::new(None),
        generation: AtomicU64::new(0),
    });

    build_menu(app)?;
    build_window(app)?;
    if !notices.is_empty() {
        // Present after the event loop starts; a dialog raised inside setup is
        // queued before the loop pumps and is not shown.
        let handle = app.clone();
        let message = notices.join("\n\n");
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            show_message(&handle, "DeepSeek Harness", &message);
        });
    }

    let handle = app.clone();
    thread::spawn(move || {
        if let Err(message) = sidecar::start(&handle) {
            show_error(&handle, "DeepSeek Harness", &message);
        }
    });
    Ok(())
}

fn build_menu(app: &AppHandle) -> Result<(), String> {
    let import = MenuItem::with_id(app, "import-runtime", "Import Runtime…", true, None::<&str>)
        .map_err(|error| format!("Cannot create the Import Runtime menu item: {error}"))?;
    let restart = MenuItem::with_id(app, "restart-runtime", "Restart", true, None::<&str>)
        .map_err(|error| format!("Cannot create the Restart menu item: {error}"))?;
    let exit = MenuItem::with_id(app, "exit", "Exit", true, None::<&str>)
        .map_err(|error| format!("Cannot create the Exit menu item: {error}"))?;
    let separator = PredefinedMenuItem::separator(app)
        .map_err(|error| format!("Cannot create the menu separator: {error}"))?;
    let about = MenuItem::with_id(app, "about", "About", true, None::<&str>)
        .map_err(|error| format!("Cannot create the About menu item: {error}"))?;
    let file = Submenu::with_items(
        app,
        "File",
        true,
        &[&import, &restart, &separator, &exit],
    )
    .map_err(|error| format!("Cannot create the File menu: {error}"))?;
    let help = Submenu::with_items(app, "Help", true, &[&about])
        .map_err(|error| format!("Cannot create the Help menu: {error}"))?;
    let menu =
        Menu::with_items(app, &[&file, &help]).map_err(|error| format!("Cannot create the menu: {error}"))?;
    app.set_menu(menu)
        .map_err(|error| format!("Cannot attach the menu: {error}"))?;
    Ok(())
}

fn build_window(app: &AppHandle) -> Result<(), String> {
    let handle = app.clone();
    tauri::WebviewWindowBuilder::new(
        app,
        "main",
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("DeepSeek Harness")
    .inner_size(1280.0, 860.0)
    .on_navigation(move |url| allow_navigation(&handle, url))
    .build()
    .map(|_| ())
    .map_err(|error| format!("Cannot create the application window: {error}"))
}

fn allow_navigation(app: &AppHandle, url: &tauri::Url) -> bool {
    let allowed = app
        .state::<AppState>()
        .origin
        .lock()
        .ok()
        .and_then(|origin| origin.clone());
    match navigation_decision(url, allowed.as_deref()) {
        NavigationDecision::Allow => true,
        NavigationDecision::External => {
            open_external(url.as_str());
            false
        }
        NavigationDecision::Block => false,
    }
}

/// What to do with one requested navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NavigationDecision {
    /// Load the URL in the application window.
    Allow,
    /// Hand the URL to the system browser and keep the current page.
    External,
    /// Refuse the navigation without an external handoff.
    Block,
}

fn navigation_decision(url: &tauri::Url, allowed_origin: Option<&str>) -> NavigationDecision {
    match url.scheme() {
        "tauri" | "about" => NavigationDecision::Allow,
        "http" | "https" => {
            if url
                .host_str()
                .is_some_and(|host| host.eq_ignore_ascii_case("tauri.localhost"))
            {
                return NavigationDecision::Allow;
            }
            if allowed_origin.is_some_and(|origin| url.origin().ascii_serialization() == origin) {
                return NavigationDecision::Allow;
            }
            NavigationDecision::External
        }
        "mailto" => NavigationDecision::External,
        _ => NavigationDecision::Block,
    }
}

fn open_external(url: &str) {
    if let Err(error) = tauri_plugin_opener::open_url(url, None::<&str>) {
        eprintln!("dsh desktop: cannot open {url}: {error}");
    }
}

fn handle_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        "import-runtime" => start_import(app.clone()),
        "restart-runtime" => confirm_restart(app.clone()),
        "about" => show_about(app),
        "exit" => app.exit(0),
        _ => {}
    }
}

fn show_about(app: &AppHandle) {
    let state = app.state::<AppState>();
    let (version, origin) = match state.runtime.read() {
        Ok(runtime) => (
            runtime.version.to_string(),
            match runtime.source {
                RuntimeSource::Seed => "bundled",
                RuntimeSource::Imported => "imported",
            },
        ),
        Err(_) => ("unavailable".to_owned(), "unknown"),
    };
    let shell = env!("CARGO_PKG_VERSION");
    show_message(
        app,
        "About DeepSeek Harness",
        &format!("Desktop shell {shell}\n\ndsh runtime {version} ({origin})"),
    );
}

fn confirm_restart(app: AppHandle) {
    app.dialog()
        .message("Restart the dsh runtime? Running work may be interrupted.")
        .title("Restart")
        .buttons(MessageDialogButtons::YesNo)
        .show(move |confirmed| {
            if confirmed {
                restart(app);
            }
        });
}

fn restart(app: AppHandle) {
    app.state::<AppState>().terminate_session();
    // shortcut: TerminateJobObject is asynchronous with respect to process
    // teardown, so a fixed 300 ms grace precedes the replacement. Wait on
    // process-tree exit instead if tool processes start lingering.
    thread::sleep(Duration::from_millis(300));
    thread::spawn(move || {
        if let Err(message) = sidecar::start(&app) {
            show_error(&app, "DeepSeek Harness", &message);
        }
    });
}

fn start_import(app: AppHandle) {
    app.dialog()
        .file()
        .add_filter("dsh runtime executable", &["exe"])
        .pick_file(move |selected| {
            let Some(selected) = selected else {
                return;
            };
            let source = match selected.into_path() {
                Ok(path) => path,
                Err(error) => {
                    show_error(
                        &app,
                        "Import Runtime",
                        &format!("Cannot use the selected path: {error}"),
                    );
                    return;
                }
            };
            let handle = app.clone();
            thread::spawn(move || import_from(&handle, &source));
        });
}

fn import_from(app: &AppHandle, source: &Path) {
    let state = app.state::<AppState>();
    let previous = state.pointer.read().ok().and_then(|pointer| {
        pointer.as_ref().and_then(|pointer| {
            let installed = state
                .layout
                .runtimes_dir()
                .join(&pointer.version)
                .join("dsh.exe")
                .is_file();
            installed.then(|| pointer.version.clone())
        })
    });
    let version = match runtime::install(
        source,
        &state.layout.runtimes_dir(),
        &state.layout.pointer_file(),
        previous,
    ) {
        Ok(version) => version,
        Err(message) => {
            show_error(app, "Import Runtime", &message);
            return;
        }
    };
    if let Ok(mut pointer) = state.pointer.write() {
        *pointer = Pointer::load(&state.layout.pointer_file()).ok().flatten();
    }
    state.reload_selected_runtime();
    state
        .log
        .append(&format!("imported dsh runtime {version} from {}", source.display()));
    let handle = app.clone();
    app.dialog()
        .message(format!("Imported dsh {version}. Restart now to use it?"))
        .title("Import Runtime")
        .buttons(MessageDialogButtons::YesNo)
        .show(move |confirmed| {
            if confirmed {
                restart(handle.clone());
            }
        });
}

fn show_message(app: &AppHandle, title: &str, message: &str) {
    app.dialog()
        .message(message)
        .title(title)
        .kind(MessageDialogKind::Warning)
        .show(|_| {});
}

fn show_error(app: &AppHandle, title: &str, message: &str) {
    app.dialog()
        .message(message)
        .title(title)
        .kind(MessageDialogKind::Error)
        .show(|_| {});
}

fn show_error_and_exit(app: &AppHandle, message: &str) {
    let handle = app.clone();
    let message = message.to_owned();
    thread::spawn(move || {
        // Let the event loop start before presenting the dialog.
        thread::sleep(Duration::from_millis(300));
        let exit = handle.clone();
        handle
            .dialog()
            .message(message)
            .title("DeepSeek Harness")
            .kind(MessageDialogKind::Error)
            .show(move |_| {
                exit.exit(1);
            });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(value: &str) -> tauri::Url {
        tauri::Url::parse(value).expect("valid URL")
    }

    #[test]
    fn allows_only_the_published_origin() {
        let allowed = Some("http://127.0.0.1:50869");
        assert_eq!(
            navigation_decision(&url("http://127.0.0.1:50869/?token=x"), allowed),
            NavigationDecision::Allow
        );
        assert_eq!(
            navigation_decision(&url("http://127.0.0.1:50869/settings"), allowed),
            NavigationDecision::Allow
        );
        assert_eq!(
            navigation_decision(&url("http://127.0.0.1:50870/"), allowed),
            NavigationDecision::External
        );
        assert_eq!(
            navigation_decision(&url("http://localhost:50869/"), allowed),
            NavigationDecision::External
        );
    }

    #[test]
    fn allows_the_splash_and_app_schemes() {
        assert_eq!(
            navigation_decision(&url("http://tauri.localhost/index.html"), None),
            NavigationDecision::Allow
        );
        assert_eq!(
            navigation_decision(&url("tauri://localhost/index.html"), None),
            NavigationDecision::Allow
        );
        assert_eq!(
            navigation_decision(&url("about:blank"), None),
            NavigationDecision::Allow
        );
    }

    #[test]
    fn routes_external_links_and_blocks_unknown_schemes() {
        assert_eq!(
            navigation_decision(&url("https://example.com/"), None),
            NavigationDecision::External
        );
        assert_eq!(
            navigation_decision(&url("mailto:dev@example.com"), None),
            NavigationDecision::External
        );
        assert_eq!(
            navigation_decision(&url("file:///C:/secret.txt"), None),
            NavigationDecision::Block
        );
    }

    #[test]
    fn loopback_is_external_before_the_sidecar_publishes() {
        assert_eq!(
            navigation_decision(&url("http://127.0.0.1:50869/"), None),
            NavigationDecision::External
        );
    }
}
