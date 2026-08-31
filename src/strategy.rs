//! Deterministic strategies. The agent is another strategy (see `crate::agent`).

use crate::types::{Action, Envelope, Item, Mode, Proposal};
use std::collections::HashMap;

pub trait Strategy: Send + Sync {
    fn name(&self) -> &'static str;
    /// Propose actions for envelopes that still have none.
    fn apply(&self, items: &mut [Item]);
}

/// Run the pipeline for a sitting. Unclaimed items stay in the queue with no
/// proposal — the user decides, or Enter ignores them.
pub fn classify(envelopes: Vec<Envelope>, mode: Mode, agent: Option<&dyn Strategy>) -> Vec<Item> {
    let mut items: Vec<Item> = envelopes
        .into_iter()
        .map(|envelope| Item {
            envelope,
            proposal: None,
        })
        .collect();

    match mode {
        Mode::Strategic => apply_deterministic(&mut items),
        Mode::Agentic => {
            if let Some(agent) = agent {
                agent.apply(&mut items);
            }
        }
        Mode::Hybrid => {
            apply_deterministic(&mut items);
            if let Some(agent) = agent {
                agent.apply(&mut items);
            }
        }
    }
    items
}

fn apply_deterministic(items: &mut [Item]) {
    for s in deterministic() {
        s.apply(items);
    }
}

fn deterministic() -> Vec<Box<dyn Strategy>> {
    vec![
        Box::new(Threads),
        Box::new(Calendar),
        Box::new(Newsletters),
        Box::new(Notifications),
    ]
}

fn set_if_free(item: &mut Item, action: Action, strategy: &'static str, reason: impl Into<String>) {
    if item.proposal.is_some() {
        return;
    }
    if action == Action::Archive && item.envelope.flagged {
        return;
    }
    item.proposal = Some(Proposal {
        action,
        reason: reason.into(),
        strategy: strategy.to_owned(),
    });
}

/// Collapse threads: keep the newest message, archive older siblings.
pub struct Threads;

impl Strategy for Threads {
    fn name(&self) -> &'static str {
        "threads"
    }

    fn apply(&self, items: &mut [Item]) {
        let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, item) in items.iter().enumerate() {
            let key = if item.envelope.thread_id.is_empty() {
                format!("subj:{}", item.envelope.normalized_subject())
            } else {
                item.envelope.thread_id.clone()
            };
            groups.entry(key).or_default().push(i);
        }
        for idxs in groups.values() {
            if idxs.len() < 2 {
                continue;
            }
            let newest = idxs
                .iter()
                .copied()
                .max_by_key(|i| items[*i].envelope.received)
                .expect("non-empty");
            for &i in idxs {
                if i == newest {
                    continue;
                }
                set_if_free(
                    &mut items[i],
                    Action::Archive,
                    "threads",
                    "older message in thread; newest kept",
                );
            }
        }
    }
}

/// Calendar *responses* (accepted/declined/tentative). Meeting requests stay.
pub struct Calendar;

const CAL_PREFIXES: &[&str] = &[
    "accepted:",
    "declined:",
    "tentative:",
    "canceled:",
    "cancelled:",
    "aceptado:",
    "rechazado:",
    "provisional:",
    "cancelado:",
    "cancelada:",
];

const REQUEST_HINTS: &[&str] = &[
    "invitation:",
    "meeting request",
    "invitación:",
    "invitacion:",
    "please respond",
];

impl Strategy for Calendar {
    fn name(&self) -> &'static str {
        "calendar"
    }

    fn apply(&self, items: &mut [Item]) {
        for item in items.iter_mut() {
            let subj = item.envelope.subject.to_ascii_lowercase();
            if REQUEST_HINTS.iter().any(|h| subj.contains(h)) {
                continue;
            }
            if CAL_PREFIXES.iter().any(|p| subj.starts_with(p)) {
                set_if_free(
                    item,
                    Action::Archive,
                    "calendar",
                    "calendar response, not a meeting request",
                );
            }
        }
    }
}

/// List-Id / bulk precedence. Distinct from notifications so grouped review
/// can treat newsletters as a class.
pub struct Newsletters;

impl Strategy for Newsletters {
    fn name(&self) -> &'static str {
        "newsletters"
    }

    fn apply(&self, items: &mut [Item]) {
        for item in items.iter_mut() {
            let list_id = item.envelope.header("List-Id").is_some();
            let unsub = item.envelope.header("List-Unsubscribe").is_some();
            let bulk = item
                .envelope
                .header("Precedence")
                .is_some_and(|v| v.eq_ignore_ascii_case("bulk") || v.eq_ignore_ascii_case("list"));
            if list_id || (unsub && bulk) {
                set_if_free(
                    item,
                    Action::Archive,
                    "newsletters",
                    "list/bulk headers",
                );
            }
        }
    }
}

/// Machine mail: Auto-Submitted, no-reply localparts. Transport headers such as
/// Received / Return-Path never select. Human mail that happens to be
/// auto-stamped is why this is a *proposal*, not an apply.
pub struct Notifications;

const NOREPLY: &[&str] = &[
    "noreply",
    "no-reply",
    "no_reply",
    "notify",
    "notification",
    "notifications",
    "alert",
    "alerts",
    "mailer-daemon",
    "postmaster",
];

impl Strategy for Notifications {
    fn name(&self) -> &'static str {
        "notifications"
    }

    fn apply(&self, items: &mut [Item]) {
        for item in items.iter_mut() {
            let auto = item.envelope.header("Auto-Submitted").is_some_and(|v| {
                !v.eq_ignore_ascii_case("no")
            });
            let local = item.envelope.from_localpart.to_ascii_lowercase();
            let noreply = NOREPLY.iter().any(|n| local == *n || local.starts_with(&format!("{n}+")));
            if auto || noreply {
                set_if_free(
                    item,
                    Action::Archive,
                    "notifications",
                    "auto-submitted or no-reply sender",
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Envelope;
    use chrono::{TimeZone, Utc};

    fn env(
        id: &str,
        thread: &str,
        day: u32,
        subject: &str,
        from: &str,
        headers: &[(&str, &str)],
    ) -> Envelope {
        let at = from.find('@').unwrap_or(from.len());
        Envelope {
            id: id.into(),
            subject: subject.into(),
            from: from.into(),
            from_localpart: from[..at].into(),
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

    #[test]
    fn threads_keep_newest() {
        let mail = vec![
            env("1", "t1", 1, "Hello", "a@example.com", &[]),
            env("2", "t1", 3, "Re: Hello", "b@example.com", &[]),
            env("3", "t1", 2, "Re: Hello", "a@example.com", &[]),
        ];
        let items = classify(mail, Mode::Strategic, None);
        let archived: Vec<_> = items
            .iter()
            .filter(|i| i.proposed_action() == Some(Action::Archive))
            .map(|i| i.envelope.id.as_str())
            .collect();
        assert_eq!(archived, ["1", "3"]);
        assert!(items[1].proposal.is_none()); // newest, day 3
    }

    #[test]
    fn flagged_never_auto_archived() {
        let mut mail = env("1", "t1", 1, "old", "a@example.com", &[]);
        mail.flagged = true;
        let newer = env("2", "t1", 2, "new", "a@example.com", &[]);
        let items = classify(vec![mail, newer], Mode::Strategic, None);
        assert!(items[0].proposal.is_none());
    }

    #[test]
    fn calendar_archives_responses_keeps_invites() {
        let mail = vec![
            env("1", "c1", 1, "Accepted: Standup", "cal@example.com", &[]),
            env("2", "c2", 1, "Invitation: Standup", "cal@example.com", &[]),
            env("3", "c3", 1, "Aceptado: Lunch", "cal@example.com", &[]),
        ];
        let items = classify(mail, Mode::Strategic, None);
        assert_eq!(items[0].proposed_action(), Some(Action::Archive));
        assert!(items[1].proposal.is_none());
        assert_eq!(items[2].proposed_action(), Some(Action::Archive));
        assert_eq!(items[0].proposal.as_ref().unwrap().strategy, "calendar");
    }

    #[test]
    fn notifications_ignore_transport_headers() {
        let mail = vec![
            env(
                "1",
                "n",
                1,
                "Balance",
                "noreply@example.com",
                &[("Auto-Submitted", "auto-generated")],
            ),
            env(
                "2",
                "n",
                1,
                "Hi",
                "human@example.com",
                &[("Received", "from nowhere"), ("Return-Path", "<>")],
            ),
        ];
        let items = classify(mail, Mode::Strategic, None);
        assert_eq!(items[0].proposed_action(), Some(Action::Archive));
        assert!(items[1].proposal.is_none());
    }

    #[test]
    fn newsletters_use_list_headers() {
        let mail = env(
            "1",
            "l",
            1,
            "Weekly",
            "news@example.com",
            &[("List-Id", "<weekly.example.com>"), ("List-Unsubscribe", "<https://example.com/u>")],
        );
        let items = classify(vec![mail], Mode::Strategic, None);
        assert_eq!(items[0].proposal.as_ref().unwrap().strategy, "newsletters");
    }

    struct StubAgent;

    impl Strategy for StubAgent {
        fn name(&self) -> &'static str {
            "agent"
        }
        fn apply(&self, items: &mut [Item]) {
            for item in items {
                set_if_free(item, Action::Archive, "agent", "stub");
            }
        }
    }

    #[test]
    fn hybrid_agent_only_gets_leftovers() {
        let mail = vec![
            env("1", "cal", 1, "Accepted: X", "cal@example.com", &[]),
            env("2", "human", 1, "Please review", "human@example.com", &[]),
        ];
        let items = classify(mail, Mode::Hybrid, Some(&StubAgent));
        assert_eq!(items[0].proposal.as_ref().unwrap().strategy, "calendar");
        assert_eq!(items[1].proposal.as_ref().unwrap().strategy, "agent");
    }

    #[test]
    fn agentic_skips_deterministic() {
        let mail = vec![env("1", "t", 1, "Accepted: X", "cal@example.com", &[])];
        let items = classify(mail, Mode::Agentic, Some(&StubAgent));
        assert_eq!(items[0].proposal.as_ref().unwrap().strategy, "agent");
    }
}
