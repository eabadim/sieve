//! Shared types: envelopes, proposed actions, run mode.

use chrono::{DateTime, Utc};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Deterministic strategies only.
    Strategic,
    /// Agent strategy classifies everything.
    Agentic,
    /// Deterministic first, agent on leftovers.
    Hybrid,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Strategic => "strategic",
            Self::Agentic => "agentic",
            Self::Hybrid => "hybrid",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Archive,
    Flag,
    /// Leave in the inbox; drop from the queue without mutating.
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    pub id: String,
    pub subject: String,
    pub from: String,
    pub from_localpart: String,
    pub received: DateTime<Utc>,
    pub unread: bool,
    pub flagged: bool,
    /// Provider thread key (IMAP REFERENCES / Graph conversation / Gmail thread).
    pub thread_id: String,
    pub headers: Vec<(String, String)>,
}

impl Envelope {
    pub fn header(&self, name: &str) -> Option<&str> {
        let want = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(&want))
            .map(|(_, v)| v.as_str())
    }

    pub fn normalized_subject(&self) -> String {
        let mut s = self.subject.trim().to_owned();
        let prefixes = [
            "re:", "fw:", "fwd:", "aw:", "wg:", "sv:", "vs:", "rif:", "enc:",
        ];
        loop {
            let lower = s.to_ascii_lowercase();
            let mut stripped = false;
            for p in prefixes {
                if lower.starts_with(p) {
                    s = s[p.len()..].trim().to_owned();
                    stripped = true;
                    break;
                }
            }
            if !stripped {
                break;
            }
        }
        s
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    pub action: Action,
    pub reason: String,
    pub strategy: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub envelope: Envelope,
    pub proposal: Option<Proposal>,
}

impl Item {
    pub fn proposed_action(&self) -> Option<Action> {
        self.proposal.as_ref().map(|p| p.action)
    }
}
