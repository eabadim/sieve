//! Mailbox I/O. Live path shells out to Himalaya; demo is in-process fixtures.

use crate::types::Envelope;
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use serde::Deserialize;
use std::process::Command;

pub trait Mailbox {
    fn list_inbox(&self) -> Result<Vec<Envelope>>;
    fn archive(&self, ids: &[String]) -> Result<()>;
    fn flag(&self, ids: &[String]) -> Result<()>;
}

/// Himalaya CLI adapter. Account is a Himalaya account *name*, never an address.
pub struct Himalaya {
    pub account: String,
    pub bin: String,
}

impl Default for Himalaya {
    fn default() -> Self {
        Self {
            account: String::new(),
            bin: "himalaya".into(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct HimEnvelope {
    id: serde_json::Value,
    #[serde(default)]
    subject: Option<String>,
    #[serde(default)]
    from: HimFrom,
    #[serde(default)]
    date: Option<String>,
    #[serde(default)]
    flags: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct HimFrom {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    addr: Option<String>,
    #[serde(default)]
    address: Option<String>,
}

impl Mailbox for Himalaya {
    fn list_inbox(&self) -> Result<Vec<Envelope>> {
        let mut cmd = Command::new(&self.bin);
        cmd.args(["envelope", "list", "--json"]);
        if !self.account.is_empty() {
            cmd.args(["--account", &self.account]);
        }
        let out = cmd.output().context("running himalaya envelope list")?;
        if !out.status.success() {
            return Err(anyhow!(
                "himalaya failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        let rows: Vec<HimEnvelope> =
            serde_json::from_slice(&out.stdout).context("parsing himalaya json")?;
        Ok(rows.into_iter().map(into_envelope).collect())
    }

    fn archive(&self, ids: &[String]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut cmd = Command::new(&self.bin);
        cmd.args(["message", "move", "--json", "-m", "Archive"]);
        if !self.account.is_empty() {
            cmd.args(["--account", &self.account]);
        }
        cmd.args(ids);
        let out = cmd.output().context("running himalaya message move")?;
        if !out.status.success() {
            return Err(anyhow!(
                "himalaya move failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        Ok(())
    }

    fn flag(&self, ids: &[String]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut cmd = Command::new(&self.bin);
        cmd.args(["flag", "add", "flagged"]);
        if !self.account.is_empty() {
            cmd.args(["--account", &self.account]);
        }
        cmd.args(ids);
        let out = cmd.output().context("running himalaya flag add")?;
        if !out.status.success() {
            return Err(anyhow!(
                "himalaya flag failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        Ok(())
    }
}

fn parse_date(s: &str) -> DateTime<Utc> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return dt.with_timezone(&Utc);
    }
    if let Ok(dt) = DateTime::parse_from_rfc2822(s) {
        return dt.with_timezone(&Utc);
    }
    Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap()
}

fn into_envelope(h: HimEnvelope) -> Envelope {
    let id = match h.id {
        serde_json::Value::String(s) => s,
        other => other.to_string(),
    };
    let addr = h.from.addr.or(h.from.address).unwrap_or_default();
    let from = match h.from.name {
        Some(n) if !n.is_empty() => format!("{n} <{addr}>"),
        _ => addr.clone(),
    };
    let local = addr.split('@').next().unwrap_or("").to_owned();
    let received = h.date.as_deref().map(parse_date).unwrap_or_else(|| {
        Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap()
    });
    let flags = h
        .flags
        .iter()
        .map(|f| f.to_ascii_lowercase())
        .collect::<Vec<_>>();
    Envelope {
        thread_id: id.clone(),
        id,
        subject: h.subject.unwrap_or_default(),
        from,
        from_localpart: local,
        received,
        unread: !flags.iter().any(|f| f == "seen" || f == "read"),
        flagged: flags.iter().any(|f| f == "flagged"),
        headers: vec![],
    }
}

/// In-memory mailbox for `--demo` and tests. Senders are example.com only.
pub struct Demo {
    pub envelopes: Vec<Envelope>,
}

impl Demo {
    pub fn fixtures() -> Self {
        let mut mail = vec![
            demo("1", "t-hello", 1, "Project update", "alex@example.com", &[]),
            demo(
                "2",
                "t-hello",
                2,
                "Re: Project update",
                "sam@example.com",
                &[],
            ),
            demo(
                "3",
                "cal",
                3,
                "Accepted: Weekly sync",
                "calendar@example.com",
                &[],
            ),
            demo(
                "4",
                "cal",
                3,
                "Invitation: Weekly sync",
                "calendar@example.com",
                &[],
            ),
            demo(
                "5",
                "news",
                4,
                "This week in rust",
                "news@example.com",
                &[
                    ("List-Id", "<weekly.example.com>"),
                    ("List-Unsubscribe", "<https://example.com/unsub>"),
                ],
            ),
            demo(
                "6",
                "note",
                4,
                "Your invoice is ready",
                "noreply@example.com",
                &[("Auto-Submitted", "auto-generated")],
            ),
            demo(
                "7",
                "human",
                5,
                "Can you review the draft?",
                "human@example.com",
                &[],
            ),
            demo("8", "t-hello", 1, "Keep me", "alex@example.com", &[]),
        ];
        if let Some(e) = mail.iter_mut().find(|e| e.id == "8") {
            e.flagged = true;
        }
        Self { envelopes: mail }
    }
}

fn demo(
    id: &str,
    thread: &str,
    day: u32,
    subject: &str,
    from: &str,
    headers: &[(&str, &str)],
) -> Envelope {
    let local = from.split('@').next().unwrap_or("").to_owned();
    Envelope {
        id: id.into(),
        subject: subject.into(),
        from: from.into(),
        from_localpart: local,
        received: Utc.with_ymd_and_hms(2026, 8, day, 12, 0, 0).unwrap(),
        unread: true,
        flagged: false,
        thread_id: thread.into(),
        headers: headers
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
    }
}

impl Mailbox for Demo {
    fn list_inbox(&self) -> Result<Vec<Envelope>> {
        Ok(self.envelopes.clone())
    }

    fn archive(&self, _ids: &[String]) -> Result<()> {
        Ok(())
    }

    fn flag(&self, _ids: &[String]) -> Result<()> {
        Ok(())
    }
}
