//! Shell configuration stored under the local application data directory.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Directory and file name of the shell configuration.
pub const CONFIG_FILE: &str = "config.toml";

/// Validated shell configuration.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Config {
    /// Directory the dsh sidecar starts in.
    pub workspace: PathBuf,
    /// Harness home forwarded as `DSH_HOME`; omitted uses the CLI default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dsh_home: Option<PathBuf>,
    /// Toolchain override for the bundled Node directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_dir: Option<PathBuf>,
    /// Toolchain override for the bundled pnpm directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pnpm_dir: Option<PathBuf>,
}

/// Configuration returned by first-run creation or a normal load.
#[derive(Debug)]
pub struct LoadedConfig {
    /// Parsed configuration.
    pub config: Config,
    /// Whether this load created the file.
    pub created: bool,
}

/// Load the configuration, creating it with `default_workspace` on first run.
///
/// An existing but invalid file is reported instead of replaced, so a user
/// typo never destroys the configured workspace or Harness home.
///
/// @param path - configuration file path.
/// @param default_workspace - workspace written by first-run creation.
/// @returns the parsed configuration and whether creation happened.
pub fn load_or_create(path: &Path, default_workspace: &Path) -> Result<LoadedConfig, String> {
    match fs::read_to_string(path) {
        Ok(contents) => {
            let config = toml::from_str(&contents)
                .map_err(|error| format!("invalid configuration at {}: {error}", path.display()))?;
            Ok(LoadedConfig { config, created: false })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let config = Config {
                workspace: default_workspace.to_path_buf(),
                dsh_home: None,
                node_dir: None,
                pnpm_dir: None,
            };
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
            }
            let contents = toml::to_string_pretty(&config)
                .map_err(|error| format!("cannot serialize {}: {error}", path.display()))?;
            fs::write(path, contents)
                .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
            Ok(LoadedConfig { config, created: true })
        }
        Err(error) => Err(format!("cannot read {}: {error}", path.display())),
    }
}

/// The workspace used when the user has not configured one.
///
/// @returns the user profile directory, or the process directory when it is absent.
pub fn default_workspace() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
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
                .join(format!("dsh-desktop-config-{}-{index}", std::process::id()));
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

    #[test]
    fn first_run_writes_defaults() {
        let dir = TempDir::new();
        let file = dir.path().join(CONFIG_FILE);
        let workspace = dir.path().join("workspace");
        let loaded = load_or_create(&file, &workspace).expect("create configuration");
        assert!(loaded.created);
        assert_eq!(loaded.config.workspace, workspace);
        assert_eq!(loaded.config.dsh_home, None);
        assert!(file.is_file());
        let reloaded = load_or_create(&file, &dir.path().join("other")).expect("reload configuration");
        assert!(!reloaded.created);
        assert_eq!(reloaded.config.workspace, workspace);
    }

    #[test]
    fn existing_file_overrides_defaults() {
        let dir = TempDir::new();
        let file = dir.path().join(CONFIG_FILE);
        let workspace = dir.path().join("configured");
        fs::write(
            &file,
            format!(
                "workspace = {:?}\ndsh_home = {:?}\nnode_dir = \"C:/tools/node\"\n",
                workspace.display().to_string(),
                dir.path().join("home").display().to_string()
            ),
        )
        .expect("write configuration");
        let loaded = load_or_create(&file, &dir.path().join("ignored")).expect("load configuration");
        assert!(!loaded.created);
        assert_eq!(loaded.config.workspace, workspace);
        assert_eq!(loaded.config.dsh_home, Some(dir.path().join("home")));
        assert_eq!(loaded.config.node_dir, Some(PathBuf::from("C:/tools/node")));
        assert_eq!(loaded.config.pnpm_dir, None);
    }

    #[test]
    fn invalid_file_is_reported_not_replaced() {
        let dir = TempDir::new();
        let file = dir.path().join(CONFIG_FILE);
        fs::write(&file, "workspace = 42\n").expect("write invalid configuration");
        let error = load_or_create(&file, dir.path()).expect_err("invalid configuration fails");
        assert!(error.contains("invalid configuration"), "unexpected error: {error}");
        assert_eq!(fs::read_to_string(&file).unwrap(), "workspace = 42\n");
    }
}
