//! Resource and per-user path layout for development and installed launches.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Bundled single-file runtime file name.
pub const RUNTIME_EXE: &str = "deepseek-harness-sdk-runtime-win-x64.exe";
/// Pointer file recording the selected installed runtime.
pub const POINTER_FILE: &str = "current.json";

/// Resolve the resource root that holds `runtime/`, `node/`, and `pnpm/`.
///
/// Development runs read the checkout's generated resources; installed runs
/// read the bundle resources beside the executable.
///
/// @param dev - whether this is a development build.
/// @param manifest_dir - the crate directory compiled into the development binary.
/// @param resource_dir - the Tauri resource directory of the running installation.
pub fn resources_root(dev: bool, manifest_dir: &Path, resource_dir: &Path) -> PathBuf {
    if dev {
        manifest_dir.join("resources")
    } else {
        resource_dir.join("resources")
    }
}

/// Resolved paths for one launch.
#[derive(Debug, Clone)]
pub struct Layout {
    /// Directory holding the bundled toolchain and seed runtime.
    pub resources: PathBuf,
    /// Per-user state directory.
    pub local_data: PathBuf,
}

impl Layout {
    /// @param local_data - per-user state directory.
    /// @param resources - directory holding `runtime/`, `node/`, and `pnpm/`.
    pub fn new(local_data: PathBuf, resources: PathBuf) -> Self {
        Self { resources, local_data }
    }

    /// @returns the bundled seed runtime executable.
    pub fn seed_runtime(&self) -> PathBuf {
        self.resources.join("runtime").join(RUNTIME_EXE)
    }

    /// @returns the bundled Node directory.
    pub fn node_dir(&self) -> PathBuf {
        self.resources.join("node")
    }

    /// @returns the bundled pnpm directory.
    pub fn pnpm_dir(&self) -> PathBuf {
        self.resources.join("pnpm")
    }

    /// @returns the directory holding imported runtime versions.
    pub fn runtimes_dir(&self) -> PathBuf {
        self.local_data.join("runtimes")
    }

    /// @returns the selected-runtime pointer file.
    pub fn pointer_file(&self) -> PathBuf {
        self.runtimes_dir().join(POINTER_FILE)
    }

    /// @returns the shell configuration file.
    pub fn config_file(&self) -> PathBuf {
        self.local_data.join(crate::config::CONFIG_FILE)
    }

    /// @returns the sidecar stderr log file.
    pub fn log_file(&self) -> PathBuf {
        self.local_data.join("logs").join("sidecar.log")
    }
}

/// Resolve the Harness home exactly as dsh will.
///
/// @param configured - `dsh_home` from the shell configuration.
/// @param inherited - `DSH_HOME` inherited from the shell's environment.
/// @returns the configured home, then the inherited home, then the user default.
pub fn resolve_harness_home(configured: Option<&Path>, inherited: Option<&OsStr>) -> PathBuf {
    configured
        .map(Path::to_path_buf)
        .or_else(|| inherited.map(PathBuf::from))
        .unwrap_or_else(default_dsh_home)
}

/// The Harness home dsh uses when `DSH_HOME` is not set.
///
/// @returns `%USERPROFILE%\.dsh`, or `.dsh` when the profile directory is unknown.
pub fn default_dsh_home() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .map(|home| PathBuf::from(home).join(".dsh"))
        .unwrap_or_else(|| PathBuf::from(".dsh"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harness_home_prefers_configured_then_inherited() {
        let configured = Path::new("C:/configured");
        let inherited = OsStr::new("C:/inherited");
        assert_eq!(
            resolve_harness_home(Some(configured), Some(inherited)),
            configured.to_path_buf()
        );
        assert_eq!(
            resolve_harness_home(None, Some(inherited)),
            PathBuf::from(inherited)
        );
        assert_eq!(
            resolve_harness_home(None, None),
            default_dsh_home()
        );
    }

    #[test]
    fn development_layout_reads_checkout_resources() {
        let manifest = Path::new("C:/repo/apps/desktop-tauri");
        let target = Path::new("C:/repo/apps/desktop-tauri/target/debug");
        assert_eq!(
            resources_root(true, manifest, target),
            PathBuf::from("C:/repo/apps/desktop-tauri/resources")
        );
    }

    #[test]
    fn installed_layout_reads_bundle_resources() {
        let manifest = Path::new("C:/ignored");
        let install = Path::new("C:/Users/me/AppData/Local/DeepSeek Harness");
        assert_eq!(
            resources_root(false, manifest, install),
            install.join("resources")
        );
    }

    #[test]
    fn seed_runtime_and_pointer_use_their_roots() {
        let layout = Layout::new(
            PathBuf::from("C:/Users/me/AppData/Local/ai.deepseek.harness.desktop-tauri"),
            PathBuf::from("C:/Users/me/AppData/Local/DeepSeek Harness/resources"),
        );
        assert_eq!(
            layout.seed_runtime(),
            PathBuf::from("C:/Users/me/AppData/Local/DeepSeek Harness/resources/runtime")
                .join(RUNTIME_EXE)
        );
        assert_eq!(
            layout.runtimes_dir().join("1.2.3"),
            PathBuf::from("C:/Users/me/AppData/Local/ai.deepseek.harness.desktop-tauri/runtimes/1.2.3")
        );
        assert_eq!(
            layout.pointer_file(),
            PathBuf::from("C:/Users/me/AppData/Local/ai.deepseek.harness.desktop-tauri/runtimes")
                .join(POINTER_FILE)
        );
    }
}
