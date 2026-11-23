//! Sidecar launch, token-URL handshake, and process-tree supervision.

use std::collections::VecDeque;
use std::env;
use std::io::{BufRead, BufReader};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

use crate::app::{AppState, SelectedRuntime};
use crate::config::Config;
use crate::job::Job;
use crate::logging::redact;
use crate::paths::{self, Layout};

/// Maximum wait for the sidecar to publish its tokenized URL.
pub const URL_WAIT: Duration = Duration::from_secs(90);
/// Retained stderr lines shown when startup fails.
const TAIL_LINES: usize = 40;

/// One supervised sidecar process.
pub struct Session {
    /// Job owning the whole sidecar tree.
    pub job: Arc<Job>,
    /// Launch identifier used to ignore stale supervisors.
    pub generation: u64,
}

impl Session {
    /// Terminate the sidecar tree.
    pub fn terminate(&self) {
        self.job.terminate();
    }
}

/// Start the dsh sidecar and load its Web UI once the tokenized URL appears.
///
/// Returns after the sidecar exits: on success this thread supervises the
/// process, and on failure the tree has already been terminated.
///
/// @param app - application handle used for the window and shared state.
pub fn start(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let config = state.config.clone();
    let layout = state.layout.clone();
    let log = state.log.clone();
    let runtime = state
        .runtime
        .read()
        .map_err(|_| "The runtime selection is unavailable.")?
        .clone();

    if !config.workspace.is_dir() {
        return Err(format!(
            "The workspace directory does not exist: {}.\n\nUpdate `workspace` in {}.",
            config.workspace.display(),
            layout.config_file().display()
        ));
    }
    let toolchain = toolchain_path(&config, &layout)?;
    let generation = state.next_generation();
    let port = free_port()
        .map_err(|error| format!("Cannot find a free port for the dsh runtime: {error}"))?;
    let profile_directory = paths::resolve_harness_home(
        config.dsh_home.as_deref(),
        env::var_os("DSH_HOME").as_deref(),
    )
    .join("profiles")
    .join("desktop-tauri");
    let first_launch = !profile_initialized(&profile_directory);

    let mut arguments = vec![
        "--profile".to_owned(),
        "desktop-tauri".to_owned(),
    ];
    if first_launch {
        arguments.push("--from-default-profile".to_owned());
        arguments.push("web".to_owned());
    }
    arguments.push("--no-open".to_owned());
    arguments.push("--port".to_owned());
    arguments.push(port.to_string());
    log.append(&format!(
        "starting {} {} (profile initialization: {first_launch})",
        runtime.exe.display(),
        arguments.join(" ")
    ));

    let mut command = Command::new(&runtime.exe);
    command.current_dir(&config.workspace);
    command.args(&arguments);
    command.env("PATH", toolchain);
    if let Some(home) = &config.dsh_home {
        command.env("DSH_HOME", home);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.creation_flags(CREATE_NO_WINDOW);

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return failed_start(
                &state,
                generation,
                &runtime,
                format!("Cannot start {}: {error}", runtime.exe.display()),
            );
        }
    };
    let job = match Job::create() {
        Ok(job) => Arc::new(job),
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return failed_start(
                &state,
                generation,
                &runtime,
                format!("Cannot supervise the dsh process: {error}"),
            );
        }
    };
    if let Err(error) = job.assign(&child) {
        let _ = child.kill();
        let _ = child.wait();
        return failed_start(
            &state,
            generation,
            &runtime,
            format!("Cannot assign the dsh process to its process job: {error}"),
        );
    }

    let (sender, receiver) = mpsc::channel::<String>();
    let stdout = child
        .stdout
        .take()
        .ok_or("The dsh process has no standard output.")?;
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            match line {
                Ok(line) => {
                    if sender.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let tail = Arc::new(Mutex::new(VecDeque::new()));
    let tail_for_reader = Arc::clone(&tail);
    let stderr = child
        .stderr
        .take()
        .ok_or("The dsh process has no standard error.")?;
    thread::spawn(move || {
        for line in BufReader::new(stderr).lines() {
            let Ok(line) = line else { break };
            // Stdout carries the tokenized URL and is never passed here.
            log.append(&line);
            let redacted = redact(&line);
            if let Ok(mut tail) = tail_for_reader.lock() {
                if tail.len() == TAIL_LINES {
                    tail.pop_front();
                }
                tail.push_back(redacted);
            }
        }
    });

    if !state.is_latest_launch(generation) {
        job.terminate();
        let _ = child.wait();
        return Ok(());
    }
    {
        let mut slot = state
            .session
            .lock()
            .map_err(|_| "The sidecar session is unavailable.")?;
        if let Some(previous) = slot.take() {
            previous.terminate();
        }
        *slot = Some(Session {
            job: Arc::clone(&job),
            generation,
        });
    }

    let outcome = wait_for_url(app, &mut child, &receiver, &tail);
    match outcome {
        Ok(()) => {
            state.mark_runtime_stable(generation, &runtime.version.to_string());
            state
                .log
                .append(&format!("loaded the dsh Web UI on http://127.0.0.1:{port}/"));
            // Drain stdout without persisting it. Dropping the receiver would
            // stop the reader and block a chatty child on a full pipe.
            thread::spawn(move || {
                for _line in receiver {}
            });
            // Supervise until exit; a restart terminates this job and wakes us.
            let _ = child.wait();
            let _ = state.take_session(generation);
            Ok(())
        }
        Err(message) => {
            job.terminate();
            let _ = child.wait();
            if state.take_session(generation).is_none() {
                // A newer launch replaced this one; it owns the error surface.
                return Ok(());
            }
            state.restore_after_failed_start(generation, &runtime.version.to_string());
            Err(message)
        }
    }
}

/// Report or suppress a launch failure, rolling back only the newest launch.
fn failed_start(
    state: &AppState,
    generation: u64,
    runtime: &SelectedRuntime,
    message: String,
) -> Result<(), String> {
    if !state.is_latest_launch(generation) {
        return Ok(());
    }
    state.restore_after_failed_start(generation, &runtime.version.to_string());
    Err(message)
}

fn wait_for_url(
    app: &AppHandle,
    child: &mut Child,
    receiver: &Receiver<String>,
    tail: &Arc<Mutex<VecDeque<String>>>,
) -> Result<(), String> {
    let deadline = Instant::now() + URL_WAIT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(with_tail(
                format!(
                    "The dsh runtime did not publish its Web UI URL within {} seconds.",
                    URL_WAIT.as_secs()
                ),
                tail,
            ));
        }
        match receiver.recv_timeout(remaining.min(Duration::from_millis(250))) {
            Ok(line) => {
                if let Some(url) = parse_entry_url(&line) {
                    let window = app
                        .get_webview_window("main")
                        .ok_or("The application window is missing.")?;
                    let parsed = tauri::Url::parse(&url)
                        .map_err(|error| format!("The dsh runtime published an invalid URL: {error}"))?;
                    if let Ok(mut origin) = app.state::<AppState>().origin.lock() {
                        *origin = Some(parsed.origin().ascii_serialization());
                    }
                    window
                        .navigate(parsed)
                        .map_err(|error| format!("Cannot load the dsh Web UI: {error}"))?;
                    return Ok(());
                }
            }
            Err(RecvTimeoutError::Timeout) => match child.try_wait() {
                Ok(Some(status)) => {
                    return Err(with_tail(
                        format!(
                            "The dsh runtime exited before publishing its Web UI URL ({status})."
                        ),
                        tail,
                    ));
                }
                Ok(None) => {}
                Err(error) => return Err(format!("Cannot check the dsh process: {error}")),
            },
            Err(RecvTimeoutError::Disconnected) => {
                return Err(with_tail(
                    "The dsh runtime closed its output before publishing its Web UI URL."
                        .to_owned(),
                    tail,
                ));
            }
        }
    }
}

fn with_tail(message: String, tail: &Arc<Mutex<VecDeque<String>>>) -> String {
    let recent = tail
        .lock()
        .map(|lines| lines.iter().cloned().collect::<Vec<_>>().join("\n"))
        .unwrap_or_default();
    if recent.is_empty() {
        message
    } else {
        format!("{message}\n\nRecent dsh output:\n{recent}")
    }
}

/// Extract the tokenized loopback URL from one published stdout line.
///
/// @param line - a sidecar stdout line, normally `label: url`.
/// @returns the URL when it is a loopback entry carrying a `token` parameter.
pub(crate) fn parse_entry_url(line: &str) -> Option<String> {
    let labelled = line.split_once(": ").map(|(_, rest)| rest.trim());
    for candidate in [Some(line.trim()), labelled].into_iter().flatten() {
        let Ok(url) = tauri::Url::parse(candidate) else {
            continue;
        };
        if !matches!(url.scheme(), "http" | "https") {
            continue;
        }
        let Some(host) = url.host_str() else {
            continue;
        };
        if !is_loopback_host(host) {
            continue;
        }
        if !url
            .query_pairs()
            .any(|(key, _)| key.eq_ignore_ascii_case("token"))
        {
            continue;
        }
        return Some(candidate.to_owned());
    }
    None
}

/// @param host - URL host.
/// @returns whether the host names the local machine.
pub fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host == "::1"
        || host == "[::1]"
        || host
            .parse::<std::net::Ipv4Addr>()
            .is_ok_and(|address| address.is_loopback())
}

/// Whether a profile directory carries a non-empty bundle list.
///
/// An empty directory, or one pre-created by `dsh plugin`, is not an
/// initialized profile and must be initialized from the shipped template.
fn profile_initialized(directory: &Path) -> bool {
    let Ok(contents) = std::fs::read_to_string(directory.join("package.json")) else {
        return false;
    };
    let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return false;
    };
    manifest
        .get("dsh")
        .and_then(|dsh| dsh.get("profile"))
        .and_then(|profile| profile.get("bundles"))
        .and_then(|bundles| bundles.as_array())
        .is_some_and(|bundles| !bundles.is_empty())
}

fn toolchain_path(config: &Config, layout: &Layout) -> Result<String, String> {
    let node = config
        .node_dir
        .clone()
        .unwrap_or_else(|| layout.node_dir());
    let pnpm = config
        .pnpm_dir
        .clone()
        .unwrap_or_else(|| layout.pnpm_dir());
    for (label, directory) in [("Node", &node), ("pnpm", &pnpm)] {
        if !directory.is_dir() {
            return Err(format!(
                "The bundled {label} toolchain is missing at {}.\n\nRun scripts/prepare-node.ps1 from the application directory before building.",
                directory.display()
            ));
        }
    }
    let inherited = env::var_os("PATH").unwrap_or_default();
    let mut entries: Vec<PathBuf> = env::split_paths(&inherited).collect();
    entries.push(node);
    entries.push(pnpm);
    env::join_paths(entries)
        .map(|value| value.to_string_lossy().into_owned())
        .map_err(|error| format!("Cannot compose the toolchain PATH: {error}"))
}

fn free_port() -> std::io::Result<u16> {
    // shortcut: the OS-assigned port is released before dsh binds it, so
    // another process can claim it in between. Add a bounded retry on bind
    // failure if collisions become observable.
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))?;
    Ok(listener.local_addr()?.port())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_labelled_token_url() {
        let line = "dsh web: http://127.0.0.1:43123/?token=s3cret";
        assert_eq!(
            parse_entry_url(line).as_deref(),
            Some("http://127.0.0.1:43123/?token=s3cret")
        );
    }

    #[test]
    fn ignores_non_loopback_and_untokenized_lines() {
        assert_eq!(parse_entry_url("dsh web: https://example.com/?token=x"), None);
        assert_eq!(parse_entry_url("dsh web: http://127.0.0.1:43123/"), None);
        assert_eq!(parse_entry_url("ready"), None);
    }

    #[test]
    fn recognises_loopback_hosts() {
        assert!(is_loopback_host("127.0.0.1"));
        assert!(is_loopback_host("127.5.0.1"));
        assert!(is_loopback_host("localhost"));
        assert!(is_loopback_host("::1"));
        assert!(!is_loopback_host("example.com"));
    }

    #[test]
    fn initialized_profile_requires_a_non_empty_bundle_list() {
        let base = std::env::temp_dir().join(format!(
            "dsh-desktop-profile-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        let profile = base.join("desktop-tauri");
        assert!(!profile_initialized(&profile));
        std::fs::create_dir_all(&profile).expect("create profile");
        std::fs::write(
            profile.join("package.json"),
            r#"{"dsh":{"profile":{"bundles":[]}}}"#,
        )
        .expect("write empty manifest");
        assert!(!profile_initialized(&profile));
        std::fs::write(
            profile.join("package.json"),
            r#"{"dsh":{"profile":{"bundles":["@deepseek-ai/dsh-web"]}}}"#,
        )
        .expect("write initialized manifest");
        assert!(profile_initialized(&profile));
        let _ = std::fs::remove_dir_all(base);
    }
}
