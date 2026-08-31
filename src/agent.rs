//! Agent strategy: a subprocess that returns JSON proposals.
//!
//! The agent never receives tokens and never talks to the mailbox. It sees
//! envelopes (subject, from, date, headers, short id) and returns
//! `{id, action, reason}` rows. Missing binary → no-op.

use crate::strategy::Strategy;
use crate::types::{Action, Item, Proposal};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentBackend {
    Claude,
    Codex,
    OpenCode,
    Pi,
}

impl AgentBackend {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "claude" => Some(Self::Claude),
            "codex" => Some(Self::Codex),
            "opencode" => Some(Self::OpenCode),
            "pi" => Some(Self::Pi),
            _ => None,
        }
    }

    fn bin(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::OpenCode => "opencode",
            Self::Pi => "pi",
        }
    }
}

#[derive(Serialize)]
struct JobItem<'a> {
    id: &'a str,
    subject: &'a str,
    from: &'a str,
    unread: bool,
    flagged: bool,
}

#[derive(Deserialize)]
struct AgentRow {
    id: String,
    action: String,
    reason: String,
}

pub struct Agent {
    pub backend: AgentBackend,
}

impl Strategy for Agent {
    fn name(&self) -> &'static str {
        "agent"
    }

    fn apply(&self, items: &mut [Item]) {
        let raw = {
            let pending: Vec<JobItem> = items
                .iter()
                .filter(|i| i.proposal.is_none())
                .map(|i| JobItem {
                    id: &i.envelope.id,
                    subject: &i.envelope.subject,
                    from: &i.envelope.from,
                    unread: i.envelope.unread,
                    flagged: i.envelope.flagged,
                })
                .collect();
            if pending.is_empty() {
                return;
            }
            match serde_json::to_string(&pending) {
                Ok(s) => s,
                Err(_) => return,
            }
        };
        let Some(output) = run_backend(self.backend, &raw) else {
            return;
        };
        let Ok(rows) = serde_json::from_str::<Vec<AgentRow>>(&output) else {
            return;
        };
        for row in rows {
            let action = match row.action.to_ascii_lowercase().as_str() {
                "archive" => Action::Archive,
                "flag" => Action::Flag,
                "skip" | "keep" => Action::Skip,
                _ => continue,
            };
            if let Some(item) = items.iter_mut().find(|i| i.envelope.id == row.id) {
                if item.proposal.is_some() {
                    continue;
                }
                if action == Action::Archive && item.envelope.flagged {
                    continue;
                }
                item.proposal = Some(Proposal {
                    action,
                    reason: row.reason,
                    strategy: "agent".into(),
                });
            }
        }
    }
}

fn run_backend(backend: AgentBackend, json: &str) -> Option<String> {
    let bin = backend.bin();
    if which(bin).is_none() {
        return None;
    }
    // Intentionally no tools / no network flags the caller didn't ask for.
    // Prompt is the JSON; we want JSON back. Backends differ; keep this
    // conservative and treat failure as "no proposals".
    let mut child = Command::new(bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(json.as_bytes());
    }
    let out = child.wait_with_output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

fn which(bin: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(bin);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}
