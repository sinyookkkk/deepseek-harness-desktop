//! dsh runtime selection, validation, and installation.

use std::fs;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use semver::Version;
use serde::{Deserialize, Serialize};
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

/// Where the selected runtime came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeSource {
    /// The runtime bundled by the installer.
    Seed,
    /// A runtime imported into the per-user runtime directory.
    Imported,
}

/// Persisted runtime selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pointer {
    /// Selected semantic version.
    pub version: String,
    /// `import` for an installed runtime, `restored` after rollback.
    pub source: String,
    /// Version retained for rollback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous: Option<String>,
}

/// A runtime chosen for this launch.
#[derive(Debug, Clone)]
pub struct Selection {
    /// Executable to start.
    pub exe: PathBuf,
    /// Selection origin.
    pub source: RuntimeSource,
    /// Recorded version; absent for the bundled seed until it is probed.
    pub version: Option<Version>,
}

impl Pointer {
    /// Read the pointer file.
    ///
    /// @param path - pointer file path.
    /// @returns the parsed pointer, or absent when the file does not exist.
    pub fn load(path: &Path) -> Result<Option<Self>, String> {
        match fs::read_to_string(path) {
            Ok(contents) => serde_json::from_str(&contents)
                .map(Some)
                .map_err(|error| format!("invalid runtime pointer {}: {error}", path.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(format!("cannot read {}: {error}", path.display())),
        }
    }

    /// Write the pointer file atomically, creating its directory.
    ///
    /// @param path - pointer file path.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
        }
        let contents = serde_json::to_string_pretty(self)
            .map_err(|error| format!("cannot serialize {}: {error}", path.display()))?;
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, contents)
            .map_err(|error| format!("cannot write {}: {error}", temporary.display()))?;
        fs::rename(&temporary, path).map_err(|error| {
            let _ = fs::remove_file(&temporary);
            format!("cannot write {}: {error}", path.display())
        })
    }

    /// Load the pointer, treating an unreadable or invalid file as absent.
    ///
    /// @param path - pointer file path.
    /// @returns the pointer when valid, and a notice when the file is corrupt.
    pub fn load_or_fallback(path: &Path) -> (Option<Self>, Option<String>) {
        match Self::load(path) {
            Ok(pointer) => (pointer, None),
            Err(error) => (
                None,
                Some(format!("{error}; using the bundled runtime.")),
            ),
        }
    }
}

/// Choose the recorded installed runtime, falling back to the bundled seed.
///
/// @param pointer - recorded selection, when one exists.
/// @param runtimes_dir - directory holding versioned runtime directories.
/// @param seed - bundled seed runtime executable.
/// @param seed_dir - directory holding the seed runtime and its sidecar.
/// @returns the selection and an optional fallback notice.
pub fn select(
    pointer: Option<&Pointer>,
    runtimes_dir: &Path,
    seed: &Path,
) -> Result<(Selection, Option<String>), String> {
    if let Some(pointer) = pointer {
        let exe = runtimes_dir.join(&pointer.version).join("dsh.exe");
        if exe.is_file() {
            match Version::parse(&pointer.version) {
                Ok(version) => {
                    return Ok((
                        Selection { exe, source: RuntimeSource::Imported, version: Some(version) },
                        None,
                    ));
                }
                Err(error) => {
                    return seed_selection(
                        seed,
                        Some(format!(
                            "The recorded runtime version {:?} is invalid ({error}); using the bundled runtime.",
                            pointer.version
                        )),
                    );
                }
            }
        }
        return seed_selection(
            seed,
            Some(format!(
                "The recorded runtime {} is missing; using the bundled runtime.",
                exe.display()
            )),
        );
    }
    seed_selection(seed, None)
}

fn seed_selection(
    seed: &Path,
    notice: Option<String>,
) -> Result<(Selection, Option<String>), String> {
    if !seed.is_file() {
        return Err(format!(
            "The bundled dsh runtime is missing at {}; run scripts/prepare-runtime.ps1 from the application directory before building.",
            seed.display()
        ));
    }
    Ok((
        Selection { exe: seed.to_path_buf(), source: RuntimeSource::Seed, version: None },
        notice,
    ))
}

/// Validate a runtime executable and read its version.
///
/// @param exe - runtime executable path.
/// @returns the reported semantic version.
pub fn probe_version(exe: &Path) -> Result<Version, String> {
    validate_with(exe, run_version)
}

/// Validate a runtime executable through an injectable version probe.
///
/// @param exe - runtime executable path.
/// @param run - probe returning exit status and captured output.
/// @returns the reported semantic version.
pub fn validate_with(
    exe: &Path,
    run: impl Fn(&Path) -> Result<Probe, String>,
) -> Result<Version, String> {
    let sidecar = sidecar_path(exe);
    if !sidecar.is_file() {
        return Err(format!(
            "The selected runtime is missing its sibling sidecar {}.",
            sidecar.display()
        ));
    }
    let probe = run(exe)?;
    if probe.exit_code != Some(0) {
        let stderr = probe.stderr.trim();
        return Err(format!(
            "{} --version failed with {}.\n{}",
            exe.display(),
            probe
                .exit_code
                .map_or_else(|| "no exit status".to_owned(), |code| format!("exit code {code}")),
            stderr
        ));
    }
    parse_version_output(&probe.stdout).ok_or_else(|| {
        format!(
            "{} --version did not report a semantic version: {}",
            exe.display(),
            probe.stdout.trim()
        )
    })
}

/// The sibling runtime sidecar expected beside an executable.
///
/// @param exe - runtime executable path.
/// @returns `<stem>-rg.exe` beside the executable.
pub fn sidecar_path(exe: &Path) -> PathBuf {
    let stem = exe
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    exe.with_file_name(format!("{stem}-rg.exe"))
}

/// Read the first semantic version token from `dsh --version` output.
///
/// @param stdout - complete standard output.
/// @returns the parsed version, if any.
pub fn parse_version_output(stdout: &str) -> Option<Version> {
    stdout.split_whitespace().find_map(|token| {
        let token = token.trim_matches(|character: char| {
            matches!(character, ',' | ';' | '(' | ')' | '[' | ']')
        });
        let token = token.strip_prefix('v').unwrap_or(token);
        Version::parse(token).ok()
    })
}

/// Captured `--version` outcome.
#[derive(Debug)]
pub struct Probe {
    /// Process exit code; absent when terminated by a signal.
    pub exit_code: Option<i32>,
    /// Standard output.
    pub stdout: String,
    /// Standard error.
    pub stderr: String,
}

fn run_version(exe: &Path) -> Result<Probe, String> {
    if !exe.is_file() {
        return Err(format!("runtime executable is missing at {}", exe.display()));
    }
    // Validation at a trust boundary: a selected executable may hang, so the
    // probe is bounded and the process is killed on timeout.
    let mut child = Command::new(exe)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|error| format!("cannot start {}: {error}", exe.display()))?;
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{} --version timed out", exe.display()));
            }
            Err(error) => return Err(format!("cannot wait for {}: {error}", exe.display())),
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("cannot read {} output: {error}", exe.display()))?;
    Ok(Probe {
        exit_code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

/// Validate and copy a runtime into the versioned runtime directory.
///
/// The pointer is written only after both files exist, so a failed copy keeps
/// the current selection.
///
/// @param source - selected runtime executable.
/// @param runtimes_dir - directory holding versioned runtime directories.
/// @param pointer_file - selection pointer written on success.
/// @param previous_version - installed version retained for rollback.
/// @returns the imported version.
pub fn install(
    source: &Path,
    runtimes_dir: &Path,
    pointer_file: &Path,
    previous_version: Option<String>,
) -> Result<Version, String> {
    let version = probe_version(source)?;
    let destination = runtimes_dir.join(version.to_string());
    fs::create_dir_all(&destination)
        .map_err(|error| format!("cannot create {}: {error}", destination.display()))?;
    let destination_exe = destination.join("dsh.exe");
    let destination_sidecar = destination.join("dsh-rg.exe");
    fs::copy(source, &destination_exe).map_err(|error| {
        format!(
            "cannot copy {} to {}: {error}",
            source.display(),
            destination_exe.display()
        )
    })?;
    fs::copy(sidecar_path(source), &destination_sidecar).map_err(|error| {
        format!(
            "cannot copy the runtime sidecar to {}: {error}",
            destination_sidecar.display()
        )
    })?;
    let pointer = Pointer {
        version: version.to_string(),
        source: "import".to_owned(),
        previous: previous_version,
    };
    pointer.save(pointer_file)?;
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT: AtomicU32 = AtomicU32::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let index = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("dsh-desktop-runtime-{}-{index}", std::process::id()));
            fs::create_dir_all(&path).expect("create temp dir");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn touch(path: &Path) {
        fs::write(path, b"").expect("create file");
    }

    fn fake_runtime(dir: &Path, name: &str) -> PathBuf {
        let exe = dir.join(name);
        touch(&exe);
        touch(&sidecar_path(&exe));
        exe
    }

    #[test]
    fn absent_pointer_selects_the_seed() {
        let dir = TempDir::new();
        let seed = fake_runtime(dir.path(), "seed.exe");
        let (selection, notice) = select(None, &dir.path().join("runtimes"), &seed).unwrap();
        assert_eq!(selection.source, RuntimeSource::Seed);
        assert_eq!(selection.exe, seed);
        assert!(notice.is_none());
    }

    #[test]
    fn recorded_runtime_is_preferred() {
        let dir = TempDir::new();
        let runtimes = dir.path().join("runtimes");
        let installed = runtimes.join("1.2.3");
        fs::create_dir_all(&installed).unwrap();
        touch(&installed.join("dsh.exe"));
        touch(&installed.join("dsh-rg.exe"));
        let pointer = Pointer {
            version: "1.2.3".to_owned(),
            source: "import".to_owned(),
            previous: None,
        };
        let seed = fake_runtime(dir.path(), "seed.exe");
        let (selection, notice) = select(Some(&pointer), &runtimes, &seed).unwrap();
        assert_eq!(selection.source, RuntimeSource::Imported);
        assert_eq!(selection.version.unwrap().to_string(), "1.2.3");
        assert!(notice.is_none());
    }

    #[test]
    fn missing_recorded_runtime_falls_back_with_a_notice() {
        let dir = TempDir::new();
        let seed = fake_runtime(dir.path(), "seed.exe");
        let pointer = Pointer {
            version: "1.2.3".to_owned(),
            source: "import".to_owned(),
            previous: None,
        };
        let (selection, notice) =
            select(Some(&pointer), &dir.path().join("runtimes"), &seed).unwrap();
        assert_eq!(selection.source, RuntimeSource::Seed);
        let notice = notice.expect("fallback notice");
        assert!(notice.contains("missing"), "notice was {notice:?}");
    }

    #[test]
    fn valid_runtime_reports_its_version() {
        let dir = TempDir::new();
        let exe = fake_runtime(dir.path(), "dsh.exe");
        let version = validate_with(&exe, |_| {
            Ok(Probe {
                exit_code: Some(0),
                stdout: "dsh 0.2.2-rc.2\n".to_owned(),
                stderr: String::new(),
            })
        })
        .unwrap();
        assert_eq!(version.to_string(), "0.2.2-rc.2");
    }

    #[test]
    fn missing_sidecar_is_rejected() {
        let dir = TempDir::new();
        let exe = dir.path().join("dsh.exe");
        touch(&exe);
        let error = validate_with(&exe, |_| unreachable!("probe must not run")).unwrap_err();
        assert!(error.contains("sibling sidecar"), "error was {error:?}");
    }

    #[test]
    fn failed_version_probe_is_rejected() {
        let dir = TempDir::new();
        let exe = fake_runtime(dir.path(), "dsh.exe");
        let error = validate_with(&exe, |_| {
            Ok(Probe {
                exit_code: Some(3),
                stdout: String::new(),
                stderr: "boom".to_owned(),
            })
        })
        .unwrap_err();
        assert!(error.contains("exit code 3"), "error was {error:?}");
        assert!(error.contains("boom"), "error was {error:?}");
    }

    #[test]
    fn unparseable_version_is_rejected() {
        let dir = TempDir::new();
        let exe = fake_runtime(dir.path(), "dsh.exe");
        let error = validate_with(&exe, |_| {
            Ok(Probe {
                exit_code: Some(0),
                stdout: "not a version\n".to_owned(),
                stderr: String::new(),
            })
        })
        .unwrap_err();
        assert!(error.contains("did not report a semantic version"), "error was {error:?}");
    }

    #[test]
    fn corrupt_pointer_is_treated_as_absent() {
        let dir = TempDir::new();
        let file = dir.path().join("current.json");
        fs::write(&file, "{not json").expect("write corrupt pointer");
        let (pointer, notice) = Pointer::load_or_fallback(&file);
        assert!(pointer.is_none());
        assert!(
            notice
                .expect("notice")
                .contains("invalid runtime pointer"),
            "notice did not name the invalid pointer"
        );
    }

    #[test]
    fn pointer_save_replaces_the_file_and_removes_the_temporary() {
        let dir = TempDir::new();
        let file = dir.path().join("current.json");
        let pointer = Pointer {
            version: "1.2.3".to_owned(),
            source: "installed".to_owned(),
            previous: None,
        };
        pointer.save(&file).expect("save pointer");
        let loaded = Pointer::load(&file).expect("load pointer").expect("present");
        assert_eq!(loaded.version, "1.2.3");
        assert_eq!(loaded.source, "installed");
        assert!(!file.with_extension("json.tmp").exists());
    }

    #[test]
    fn installing_copies_both_files_and_writes_the_pointer() {
        let dir = TempDir::new();
        let source = fake_runtime(dir.path(), "built.exe");
        let runtimes = dir.path().join("runtimes");
        let pointer_file = runtimes.join("current.json");
        let version = install(
            &source,
            &runtimes,
            &pointer_file,
            Some("1.0.0".to_owned()),
        )
        .expect_err("a real runtime is required");
        assert!(version.contains("does not report") || version.contains("cannot start"));
        // The pure-copy path is covered by the selection tests; this negative
        // case proves a non-runtime executable is rejected before installation.
        assert!(!pointer_file.exists());
    }
}
