mod cli;
mod file;

pub use cli::{Cli, Command};

use std::path::PathBuf;

use ssh2_config::{HostParams, ParseRule, SshConfig};

use crate::device::DeviceProfile;
use crate::orientation::Orientation;

/// Default SSH user and port when not overridden by ~/.ssh/config.
const DEFAULT_SSH_USER: &str = "root";
const DEFAULT_SSH_PORT: u16 = 22;
const DEFAULT_KEY_PATH: &str = "rm-key";

/// Authentication method for SSH connection.
#[derive(Clone)]
pub enum Auth {
    Key(PathBuf),
    Password(String),
}

/// Fully resolved SSH connection target, after applying ~/.ssh/config.
#[derive(Clone)]
pub struct SshTarget {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: Auth,
}

/// Merged configuration from CLI args and TOML file.
#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub key_path: Option<String>,
    pub password: Option<String>,
    pub pen_device: String,
    pub touch_device: String,
    pub touch_only: bool,
    pub pen_only: bool,
    pub grab_input: bool,
    pub ssh_config: bool,
    pub no_palm_rejection: bool,
    pub palm_grace_ms: u64,
    pub orientation: Orientation,
}

impl Config {
    /// Load configuration by merging TOML file with CLI overrides.
    pub fn load(cli: &Cli, device: &DeviceProfile) -> Self {
        let file_config = cli
            .config
            .as_ref()
            .and_then(|p| file::load_from_path(p))
            .or_else(file::load_from_default_paths)
            .unwrap_or_default();

        Self {
            host: cli.host.clone().unwrap_or(file_config.host),
            key_path: cli.key_path.clone().or(file_config.key_path),
            password: cli.password.clone().or(file_config.password),
            pen_device: cli
                .pen_device
                .clone()
                .unwrap_or_else(|| file_config.pen_device.unwrap_or(device.pen_device.into())),
            touch_device: cli
                .touch_device
                .clone()
                .unwrap_or_else(|| file_config.touch_device.unwrap_or(device.touch_device.into())),
            touch_only: cli.touch_only || file_config.touch_only,
            pen_only: cli.pen_only || file_config.pen_only,
            grab_input: if cli.no_grab_input {
                false
            } else {
                cli.grab_input || file_config.grab_input
            },
            ssh_config: if cli.no_ssh_config {
                false
            } else {
                cli.ssh_config || file_config.ssh_config
            },
            no_palm_rejection: cli.no_palm_rejection || file_config.no_palm_rejection,
            palm_grace_ms: cli
                .palm_grace_ms
                .or(file_config.palm_grace_ms)
                .unwrap_or(500),
            orientation: cli.orientation.unwrap_or(file_config.orientation),
        }
    }

    /// Resolve the SSH connection target, applying `~/.ssh/config` when enabled.
    ///
    /// The user-supplied host is the lookup key. `HostName`, `Port`, `User`, and
    /// `IdentityFile` from the matching ssh config entry override the hardcoded
    /// defaults, but explicit CLI/TOML values (password, key path) always win.
    pub fn ssh_target(&self) -> SshTarget {
        let params = if self.ssh_config {
            ssh_config_params(&self.host)
        } else {
            None
        };

        let host = params
            .as_ref()
            .and_then(|p| p.host_name.clone())
            .unwrap_or_else(|| self.host.clone());
        let port = params
            .as_ref()
            .and_then(|p| p.port)
            .unwrap_or(DEFAULT_SSH_PORT);
        let user = params
            .as_ref()
            .and_then(|p| p.user.clone())
            .unwrap_or_else(|| DEFAULT_SSH_USER.to_string());

        let ssh_identity = params
            .as_ref()
            .and_then(|p| p.identity_file.as_ref())
            .and_then(|files| files.first().cloned());

        SshTarget {
            host,
            port,
            user,
            auth: self.resolve_auth(ssh_identity),
        }
    }

    fn resolve_auth(&self, ssh_identity: Option<PathBuf>) -> Auth {
        if let Some(ref password) = self.password {
            return Auth::Password(password.clone());
        }
        if let Some(ref path) = self.key_path {
            return Auth::Key(expand_tilde(path));
        }
        if let Some(identity) = ssh_identity {
            return Auth::Key(identity);
        }
        Auth::Key(expand_tilde(DEFAULT_KEY_PATH))
    }

    pub fn run_pen(&self) -> bool {
        !self.touch_only
    }

    pub fn run_touch(&self) -> bool {
        !self.pen_only
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.touch_only && self.pen_only {
            return Err("Cannot use both --touch-only and --pen-only");
        }
        if !self.run_pen() && !self.run_touch() {
            return Err("No input device enabled");
        }
        Ok(())
    }
}

/// Query `~/.ssh/config` for the given host alias.
///
/// Returns `None` (falling back to defaults) if the file is absent or fails to
/// parse. Parsing is lenient so unsupported/unknown directives don't abort.
fn ssh_config_params(host: &str) -> Option<HostParams> {
    match SshConfig::parse_default_file(ParseRule::ALLOW_UNKNOWN_FIELDS) {
        Ok(config) => Some(config.query(host)),
        Err(e) => {
            log::debug!("Not using ~/.ssh/config: {}", e);
            None
        }
    }
}

/// Expand a leading `~` or `~/` to the user's home directory.
fn expand_tilde(path: &str) -> PathBuf {
    if path == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home);
        }
    } else if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(path)
}
