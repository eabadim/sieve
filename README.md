# sieve

A terminal inbox sieve. Not a mail client.

You open it, drain the queue, quit. It archives, flags, and skips. It does not
compose, reply, or render HTML.

Mailbox I/O is [Himalaya](https://github.com/pimalaya/himalaya). The TUI is
Ratatui. Classification is **strategy-based**, and the agent is itself a
strategy.

## Run modes

Pick one per sitting:

| Mode | What fills the queue |
| --- | --- |
| `strategic` | Deterministic strategies only (threads, calendar, newsletters, notifications, …) |
| `agentic` | The agent strategy classifies everything |
| `hybrid` | Deterministic strategies first, agent on whatever is left |

## Confirm

Strategies propose an action. You walk the queue:

- `j` / `k` — next / previous
- `a` — archive this now
- `f` — flag this now
- `s` — skip (leave it, drop from the queue)
- `u` — undo last applied action
- `Enter` — apply **remaining** proposals
- `q` — quit (unapplied proposals are discarded)

Inbox zero in this tool means the queue is empty, not that Himalaya has no mail.

## What this is not

- Not a reader. Open your mail client if you need the body.
- Not aerc, not himalaya-tui, not mutt.
- Not a sender.

## Status

Early. `sieve --demo` runs the TUI against fixture envelopes (no mailbox).
Live Himalaya I/O and agent backends are next.

```bash
sieve --demo
sieve --demo --mode hybrid
sieve --account personal --mode strategic   # needs Himalaya (not wired yet)
```

## Safety

- Default is propose-then-confirm. `Enter` is the batch apply.
- Flagged mail is never auto-proposed as archive.
- The agent never receives mailbox tokens and never talks to the provider.
- Do not put account addresses, tokens, or tenant IDs in this repository.

## Config

Local only, not committed:

```toml
# ~/.config/sieve/config.toml
account = "personal"          # Himalaya account name
mode = "hybrid"               # strategic | agentic | hybrid
agent = "claude"              # claude | codex | opencode | pi
```

## Develop

```bash
cargo test
cargo run -- --demo
```

Requires a recent stable Rust. Himalaya is a **runtime** dependency for live
mail, not a compile-time one.
