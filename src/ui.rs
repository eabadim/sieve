use crate::app::App;
use crate::types::Action;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(5),
            Constraint::Length(2),
        ])
        .split(frame.area());

    draw_header(frame, chunks[0], app);
    draw_queue(frame, chunks[1], app);
    draw_detail(frame, chunks[2], app);
    draw_status(frame, chunks[3], app);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let title = format!(
        " sieve · {} · {} left ",
        app.mode.as_str(),
        app.items.len()
    );
    let help = " j/k  a archive  f flag  s skip  u undo  enter apply rest  q quit ";
    let p = Paragraph::new(vec![Line::from(help)])
        .block(Block::default().title(title).borders(Borders::ALL));
    frame.render_widget(p, area);
}

fn draw_queue(frame: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let mark = match item.proposed_action() {
                Some(Action::Archive) => "A",
                Some(Action::Flag) => "F",
                Some(Action::Skip) => "s",
                None => "·",
            };
            let flag = if item.envelope.flagged { "*" } else { " " };
            let strat = item
                .proposal
                .as_ref()
                .map(|p| p.strategy.as_str())
                .unwrap_or("-");
            let line = format!(
                "{mark}{flag} {:<12}  {:<28}  {}",
                strat,
                truncate(&item.envelope.from, 28),
                truncate(&item.envelope.subject, 60)
            );
            let mut style = Style::default();
            if i == app.selected {
                style = style.fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
            } else if item.proposal.is_none() {
                style = style.fg(Color::DarkGray);
            }
            ListItem::new(Line::from(Span::styled(line, style)))
        })
        .collect();
    let list = List::new(items).block(Block::default().title(" queue ").borders(Borders::ALL));
    frame.render_widget(list, area);
}

fn draw_detail(frame: &mut Frame, area: Rect, app: &App) {
    let body = match app.current() {
        None => "queue empty — inbox zero for this sitting.".to_owned(),
        Some(item) => {
            let reason = item
                .proposal
                .as_ref()
                .map(|p| format!("{} · {}", p.strategy, p.reason))
                .unwrap_or_else(|| "no proposal — decide with a/f/s".into());
            format!(
                "{}\n{}\n{}",
                item.envelope.subject, item.envelope.from, reason
            )
        }
    };
    let p = Paragraph::new(body)
        .wrap(Wrap { trim: true })
        .block(Block::default().title(" current ").borders(Borders::ALL));
    frame.render_widget(p, area);
}

fn draw_status(frame: &mut Frame, area: Rect, app: &App) {
    let p = Paragraph::new(app.status.as_str());
    frame.render_widget(p, area);
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
    t.push('…');
    t
}
