use std::{
    ffi::OsString,
    path::Path,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};

use once_cell::sync::Lazy;
use thiserror::Error;
use utils::shell::resolve_executable_path_blocking;

#[derive(Debug, Error)]
pub enum StgCliError {
    #[error("stg executable not found or not runnable")]
    NotAvailable,
    #[error("stg command failed: {0}")]
    CommandFailed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchState {
    Applied,
    Unapplied,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchEntry {
    pub name: String,
    pub description: Option<String>,
    pub state: PatchState,
    pub is_current: bool,
}

static WORKTREE_LOCKS: Lazy<Mutex<std::collections::HashMap<std::path::PathBuf, Arc<Mutex<()>>>>> =
    Lazy::new(|| Mutex::new(std::collections::HashMap::new()));

fn with_worktree_lock<T>(
    worktree_path: &Path,
    f: impl FnOnce() -> Result<T, StgCliError>,
) -> Result<T, StgCliError> {
    let lock = {
        let mut map = WORKTREE_LOCKS
            .lock()
            .map_err(|_| StgCliError::CommandFailed("worktree lock poisoned".to_string()))?;
        map.entry(worktree_path.to_path_buf())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };
    let _guard = lock
        .lock()
        .map_err(|_| StgCliError::CommandFailed("worktree lock poisoned".to_string()))?;
    f()
}

#[derive(Clone, Default)]
pub struct StgCli;

impl StgCli {
    pub fn new() -> Self {
        Self {}
    }

    fn ensure_available(&self) -> Result<(), StgCliError> {
        if resolve_executable_path_blocking("stg").is_none() {
            return Err(StgCliError::NotAvailable);
        }
        Ok(())
    }

    fn stg_impl(&self, worktree_path: &Path, args: Vec<OsString>) -> Result<String, StgCliError> {
        self.ensure_available()?;
        with_worktree_lock(worktree_path, || {
            let mut cmd = Command::new("stg");
            cmd.current_dir(worktree_path)
                .args(args)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            let output = cmd
                .output()
                .map_err(|e| StgCliError::CommandFailed(format!("failed to spawn stg: {e}")))?;
            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).to_string())
            } else {
                let mut msg = String::new();
                msg.push_str(&String::from_utf8_lossy(&output.stderr));
                if msg.trim().is_empty() {
                    msg = String::from_utf8_lossy(&output.stdout).to_string();
                }
                Err(StgCliError::CommandFailed(msg.trim().to_string()))
            }
        })
    }

    pub fn is_enabled(&self, worktree_path: &Path) -> Result<bool, StgCliError> {
        // `stg series` fails if not enabled; treat that as disabled.
        match self.stg_impl(
            worktree_path,
            vec![
                OsString::from("--color=never"),
                OsString::from("series"),
                OsString::from("--description"),
            ],
        ) {
            Ok(_) => Ok(true),
            Err(StgCliError::CommandFailed(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub fn enable(&self, worktree_path: &Path) -> Result<(), StgCliError> {
        let _ = self.stg_impl(worktree_path, vec![OsString::from("init")])?;
        Ok(())
    }

    pub fn series(&self, worktree_path: &Path) -> Result<Vec<PatchEntry>, StgCliError> {
        let out = self.stg_impl(
            worktree_path,
            vec![
                OsString::from("--color=never"),
                OsString::from("series"),
                OsString::from("--description"),
            ],
        )?;
        Ok(parse_series_description(&out))
    }

    pub fn new_patch(
        &self,
        worktree_path: &Path,
        name: Option<&str>,
        message: &str,
    ) -> Result<(), StgCliError> {
        let mut args = vec![OsString::from("new")];
        if let Some(name) = name.filter(|s| !s.trim().is_empty()) {
            args.push(OsString::from(name));
        }
        args.push(OsString::from("-m"));
        args.push(OsString::from(message));
        let _ = self.stg_impl(worktree_path, args)?;
        Ok(())
    }

    pub fn refresh(
        &self,
        worktree_path: &Path,
        paths: &[String],
        allow_dirty_index: bool,
    ) -> Result<(), StgCliError> {
        let mut args = vec![OsString::from("refresh")];
        if allow_dirty_index {
            args.push(OsString::from("--index"));
        }
        for p in paths {
            args.push(OsString::from(p));
        }
        let _ = self.stg_impl(worktree_path, args)?;
        Ok(())
    }

    pub fn goto(&self, worktree_path: &Path, patch: &str) -> Result<(), StgCliError> {
        let _ = self.stg_impl(worktree_path, vec![OsString::from("goto"), patch.into()])?;
        Ok(())
    }

    pub fn push(&self, worktree_path: &Path, range: Option<&str>) -> Result<(), StgCliError> {
        let mut args = vec![OsString::from("push")];
        if let Some(r) = range.filter(|s| !s.trim().is_empty()) {
            args.push(OsString::from(r));
        }
        let _ = self.stg_impl(worktree_path, args)?;
        Ok(())
    }

    pub fn pop(&self, worktree_path: &Path, range: Option<&str>) -> Result<(), StgCliError> {
        let mut args = vec![OsString::from("pop")];
        if let Some(r) = range.filter(|s| !s.trim().is_empty()) {
            args.push(OsString::from(r));
        }
        let _ = self.stg_impl(worktree_path, args)?;
        Ok(())
    }

    pub fn float(&self, worktree_path: &Path, patches: &[String]) -> Result<(), StgCliError> {
        let mut args = vec![OsString::from("float")];
        for p in patches {
            args.push(OsString::from(p));
        }
        let _ = self.stg_impl(worktree_path, args)?;
        Ok(())
    }

    pub fn rebase(&self, worktree_path: &Path, new_base: &str) -> Result<(), StgCliError> {
        let _ = self.stg_impl(
            worktree_path,
            vec![OsString::from("rebase"), new_base.into()],
        )?;
        Ok(())
    }

    pub fn undo(&self, worktree_path: &Path) -> Result<(), StgCliError> {
        let _ = self.stg_impl(worktree_path, vec![OsString::from("undo")])?;
        Ok(())
    }

    pub fn redo(&self, worktree_path: &Path) -> Result<(), StgCliError> {
        let _ = self.stg_impl(worktree_path, vec![OsString::from("redo")])?;
        Ok(())
    }
}

fn parse_series_description(out: &str) -> Vec<PatchEntry> {
    let mut patches = Vec::new();

    for raw in out.lines() {
        let line = raw.trim_end();
        if line.trim().is_empty() {
            continue;
        }

        // Most common formats:
        //  + patch-name  description...
        //  - patch-name  description...
        //  > patch-name  description...
        // Some versions prefix current applied patch with ">" plus applied marker; we handle any mix.
        let trimmed = line.trim_start();
        let mut chars = trimmed.chars();
        let mut is_current = false;
        let mut state: Option<PatchState> = None;
        let mut consumed = 0usize;
        for c in chars.by_ref() {
            match c {
                '>' => {
                    is_current = true;
                    consumed += 1;
                }
                '+' => {
                    state = Some(PatchState::Applied);
                    consumed += 1;
                }
                '-' => {
                    state = Some(PatchState::Unapplied);
                    consumed += 1;
                }
                ' ' | '\t' => {
                    consumed += 1;
                }
                _ => break,
            }
            // Stop once we've consumed the typical prefix region; the next token is patch name.
            if consumed > 6 {
                break;
            }
        }

        let rest = trimmed.get(consumed..).unwrap_or(trimmed).trim_start();
        if rest.is_empty() {
            continue;
        }

        let (name, desc) = match rest.split_once(char::is_whitespace) {
            Some((a, b)) => (a.trim(), b.trim()),
            None => (rest.trim(), ""),
        };
        if name.is_empty() {
            continue;
        }

        patches.push(PatchEntry {
            name: name.to_string(),
            description: (!desc.is_empty()).then(|| desc.to_string()),
            state: state.unwrap_or(PatchState::Applied),
            is_current,
        });
    }

    patches
}
