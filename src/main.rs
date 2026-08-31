use anyhow::{bail, Result};
use clap::Parser;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use sieve::agent::{Agent, AgentBackend};
use sieve::app::App;
use sieve::mailbox::{Demo, Himalaya, Mailbox};
use sieve::strategy::classify;
use sieve::types::Mode;
use sieve::ui;
use std::io::{self, stdout};
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(
    name = "sieve",
    about = "Triage TUI for email. Not a mail client.",
    version
)]
struct Cli {
    /// Himalaya account name (not an email address).
    #[arg(long, short)]
    account: Option<String>,

    /// strategic | agentic | hybrid
    #[arg(long, short, value_enum, default_value_t = Mode::Hybrid)]
    mode: Mode,

    /// Agent backend used in agentic/hybrid: claude, codex, opencode, pi
    #[arg(long)]
    agent: Option<String>,

    /// Run against fixture envelopes. No mailbox.
    #[arg(long)]
    demo: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.demo {
        return run_demo(cli.mode, cli.agent.as_deref());
    }
    let Some(account) = cli.account else {
        bail!("pass --account <himalaya-account> or --demo");
    };
    run_live(account, cli.mode, cli.agent.as_deref())
}

fn run_demo(mode: Mode, agent_name: Option<&str>) -> Result<()> {
    let mailbox = Demo::fixtures();
    let envelopes = mailbox.list_inbox()?;
    let agent = agent_from_name(agent_name, mode);
    let items = classify(
        envelopes,
        mode,
        agent.as_ref().map(|a| a as &dyn sieve::strategy::Strategy),
    );
    let mut app = App::new(items, mode);
    run_tui(&mut app, &mailbox)
}

fn run_live(account: String, mode: Mode, agent_name: Option<&str>) -> Result<()> {
    let mailbox = Himalaya {
        account,
        bin: "himalaya".into(),
    };
    let envelopes = mailbox.list_inbox()?;
    let agent = agent_from_name(agent_name, mode);
    let items = classify(
        envelopes,
        mode,
        agent.as_ref().map(|a| a as &dyn sieve::strategy::Strategy),
    );
    let mut app = App::new(items, mode);
    run_tui(&mut app, &mailbox)
}

fn agent_from_name(name: Option<&str>, mode: Mode) -> Option<Agent> {
    if matches!(mode, Mode::Strategic) {
        return None;
    }
    let backend = name
        .and_then(AgentBackend::from_name)
        .unwrap_or(AgentBackend::Claude);
    Some(Agent { backend })
}

fn run_tui(app: &mut App, mailbox: &dyn Mailbox) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = tui_loop(&mut terminal, app, mailbox);
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    result
}

fn tui_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    mailbox: &dyn Mailbox,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;
        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key.code, mailbox)?;
                }
            }
        }
        if app.quit {
            break;
        }
    }
    Ok(())
}
