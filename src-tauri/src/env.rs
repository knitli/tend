//! User shell environment capture.
//!
//! When the workbench is launched from a desktop launcher (not from a terminal),
//! its process environment is whatever the launcher provided — typically a
//! sparse PATH that doesn't include user-specific additions like
//! `~/.local/bin`, nvm shims, pyenv, or aliases set up in `~/.zshrc`.
//!
//! This module captures the environment a fresh login + interactive shell
//! would produce, so workbench-spawned sessions inherit the same PATH and
//! variables the user gets in a real terminal.
//!
//! Strategy:
//! 1. Look at `$SHELL` to pick zsh or bash.
//! 2. Run `<shell> -l -i -c "env -0 > <tempfile>"`.
//!    - `-l` sources login files (~/.zprofile, ~/.bash_profile, ~/.profile).
//!    - `-i` sources interactive rc files (~/.zshrc, ~/.bashrc).
//!    - `~/.zshenv` is sourced unconditionally by zsh in all cases.
//! 3. Read the tempfile, parse NUL-delimited KEY=VALUE pairs.
//! 4. On any failure, fall back to the current process environment.
//!
//! Captured once at workbench startup and cached in `WorkbenchState`.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tempfile::NamedTempFile;
use tokio::process::Command;
use tracing::{info, warn};

/// Maximum time to wait for the shell to dump its environment. Heavy rc files
/// (nvm, conda, oh-my-zsh) can take 1-2s; 5s is a generous ceiling.
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(5);

/// Capture the user's interactive shell environment.
///
/// Returns the inherited process environment if the capture fails or times out
/// — the workbench still works, the user just may not see PATH additions.
pub async fn capture_user_env() -> HashMap<String, String> {
    match try_capture().await {
        Ok(env) => {
            info!(
                count = env.len(),
                "captured user shell environment for spawned sessions"
            );
            env
        }
        Err(reason) => {
            warn!(
                "could not capture user shell environment ({reason}); \
                 falling back to current process env. Spawned agents may not \
                 find user-specific PATH additions"
            );
            std::env::vars().collect()
        }
    }
}

async fn try_capture() -> Result<HashMap<String, String>, String> {
    let shell_path = pick_shell();

    // Tempfile lives until dropped, after which it's removed automatically.
    let tempfile = NamedTempFile::new().map_err(|e| format!("tempfile: {e}"))?;
    let tempfile_path = tempfile.path().to_owned();

    // Build the inner shell command. Quoting the path for the shell-eval side
    // is unnecessary because tempfile paths are alphanumeric + dot/slash, but
    // we still escape defensively.
    let inner = format!("env -0 > {}", shell_quote(&tempfile_path));

    let mut cmd = Command::new(&shell_path);
    cmd.arg("-l")
        .arg("-i")
        .arg("-c")
        .arg(&inner)
        // rc-file output is noise; we only care about the tempfile contents.
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let result = tokio::time::timeout(CAPTURE_TIMEOUT, cmd.status()).await;

    let status = match result {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => return Err(format!("spawn failed: {e}")),
        Err(_) => return Err(format!("timed out after {:?}", CAPTURE_TIMEOUT)),
    };

    if !status.success() {
        return Err(format!(
            "{} exited with status {:?}",
            shell_path.display(),
            status.code()
        ));
    }

    let bytes = tokio::fs::read(&tempfile_path)
        .await
        .map_err(|e| format!("read tempfile: {e}"))?;

    if bytes.is_empty() {
        return Err("env dump was empty".to_string());
    }

    Ok(parse_env_dump(&bytes))
}

/// Pick the user's shell from `$SHELL`, falling back to `/bin/bash`.
fn pick_shell() -> std::path::PathBuf {
    if let Ok(s) = std::env::var("SHELL") {
        let path = std::path::PathBuf::from(&s);
        // We support zsh and bash explicitly. Other shells (fish, nu) have
        // different rc-file conventions and `env -0` may not be available
        // (nu's syntax differs). Accept zsh/bash by basename, else fall
        // through to bash.
        if let Some(name) = path.file_name().and_then(OsStr::to_str)
            && (name == "zsh" || name == "bash")
            && path.exists()
        {
            return path;
        }
    }
    std::path::PathBuf::from("/bin/bash")
}

/// Parse a NUL-delimited `KEY=VALUE\0KEY=VALUE\0...` dump from `env -0`.
pub(crate) fn parse_env_dump(bytes: &[u8]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for entry in bytes.split(|&b| b == 0) {
        if entry.is_empty() {
            continue;
        }
        let Ok(s) = std::str::from_utf8(entry) else {
            continue;
        };
        if let Some((k, v)) = s.split_once('=') {
            out.insert(k.to_string(), v.to_string());
        }
    }
    out
}

/// Shell-quote a path for safe inclusion in a `sh -c` string. Wraps in single
/// quotes and escapes any embedded single quotes.
fn shell_quote(path: &Path) -> String {
    let s = path.to_string_lossy();
    let escaped = s.replace('\'', "'\\''");
    format!("'{escaped}'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_dump() {
        let input = b"PATH=/usr/bin:/bin\0HOME=/home/user\0LANG=en_US.UTF-8\0";
        let env = parse_env_dump(input);
        assert_eq!(env.get("PATH"), Some(&"/usr/bin:/bin".to_string()));
        assert_eq!(env.get("HOME"), Some(&"/home/user".to_string()));
        assert_eq!(env.get("LANG"), Some(&"en_US.UTF-8".to_string()));
        assert_eq!(env.len(), 3);
    }

    #[test]
    fn parse_value_with_equals() {
        // Values can legitimately contain `=` (e.g., LS_COLORS).
        let input = b"LS_COLORS=di=01;34:ln=01;36\0";
        let env = parse_env_dump(input);
        assert_eq!(env.get("LS_COLORS"), Some(&"di=01;34:ln=01;36".to_string()));
    }

    #[test]
    fn parse_skips_empty_and_malformed() {
        // Empty entries (trailing \0) and entries without = are dropped.
        let input = b"\0PATH=/bin\0NOEQUALS\0\0HOME=/h\0";
        let env = parse_env_dump(input);
        assert_eq!(env.len(), 2);
        assert!(env.contains_key("PATH"));
        assert!(env.contains_key("HOME"));
        assert!(!env.contains_key("NOEQUALS"));
    }

    #[test]
    fn parse_empty_input_yields_empty_map() {
        assert!(parse_env_dump(b"").is_empty());
    }

    #[test]
    fn shell_quote_handles_single_quotes() {
        assert_eq!(shell_quote(Path::new("/tmp/a")), "'/tmp/a'");
        assert_eq!(shell_quote(Path::new("/t/can't")), "'/t/can'\\''t'");
    }

    #[tokio::test]
    async fn capture_returns_something() {
        // End-to-end: capture should produce a non-empty map (either real
        // shell env, or the process-env fallback). At minimum PATH should
        // be present in any sane environment.
        let env = capture_user_env().await;
        assert!(!env.is_empty(), "captured env should not be empty");
        assert!(env.contains_key("PATH") || env.contains_key("HOME"));
    }
}
