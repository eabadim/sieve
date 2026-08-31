//! Queue + key handling. Mutations go through `Mailbox`. Demo mailbox is a no-op.

use crate::mailbox::Mailbox;
use crate::types::{Action, Item, Mode};
use anyhow::Result;
use crossterm::event::KeyCode;

pub struct Undo {
    pub item: Item,
    pub action: Action,
}

pub struct App {
    pub items: Vec<Item>,
    pub selected: usize,
    pub mode: Mode,
    pub status: String,
    pub undo: Vec<Undo>,
    pub quit: bool,
    pub applied: usize,
}

impl App {
    pub fn new(items: Vec<Item>, mode: Mode) -> Self {
        let n = items.len();
        let proposed = items.iter().filter(|i| i.proposal.is_some()).count();
        Self {
            items,
            selected: 0,
            mode,
            status: format!("{n} in queue, {proposed} proposed"),
            undo: vec![],
            quit: false,
            applied: 0,
        }
    }

    pub fn current(&self) -> Option<&Item> {
        self.items.get(self.selected)
    }

    pub fn handle_key(&mut self, key: KeyCode, mailbox: &dyn Mailbox) -> Result<()> {
        match key {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('j') | KeyCode::Down => self.move_sel(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_sel(-1),
            KeyCode::Char('a') => self.apply_one(Action::Archive, mailbox)?,
            KeyCode::Char('f') => self.apply_one(Action::Flag, mailbox)?,
            KeyCode::Char('s') => self.apply_one(Action::Skip, mailbox)?,
            KeyCode::Char('u') => self.undo_last(),
            KeyCode::Enter => self.apply_remaining(mailbox)?,
            _ => {}
        }
        if self.items.is_empty() {
            self.status = format!("inbox zero · {} applied · q to quit", self.applied);
        }
        Ok(())
    }

    fn move_sel(&mut self, delta: i32) {
        if self.items.is_empty() {
            return;
        }
        let n = self.items.len() as i32;
        let next = (self.selected as i32 + delta).rem_euclid(n) as usize;
        self.selected = next;
    }

    fn apply_one(&mut self, action: Action, mailbox: &dyn Mailbox) -> Result<()> {
        if self.selected >= self.items.len() {
            return Ok(());
        }
        let item = self.items.remove(self.selected);
        if self.selected >= self.items.len() && !self.items.is_empty() {
            self.selected = self.items.len() - 1;
        }
        match action {
            Action::Archive => mailbox.archive(&[item.envelope.id.clone()])?,
            Action::Flag => mailbox.flag(&[item.envelope.id.clone()])?,
            Action::Skip => {}
        }
        self.applied += 1;
        self.status = format!("{:?} {}", action, item.envelope.subject);
        self.undo.push(Undo { item, action });
        Ok(())
    }

    fn apply_remaining(&mut self, mailbox: &dyn Mailbox) -> Result<()> {
        let mut archive = vec![];
        let mut flag = vec![];
        let mut kept = vec![];
        let mut undos = vec![];
        for item in self.items.drain(..) {
            match item.proposed_action() {
                Some(Action::Archive) => {
                    archive.push(item.envelope.id.clone());
                    undos.push(Undo {
                        item,
                        action: Action::Archive,
                    });
                }
                Some(Action::Flag) => {
                    flag.push(item.envelope.id.clone());
                    undos.push(Undo {
                        item,
                        action: Action::Flag,
                    });
                }
                Some(Action::Skip) | None => kept.push(item),
            }
        }
        mailbox.archive(&archive)?;
        mailbox.flag(&flag)?;
        let n = archive.len() + flag.len();
        self.applied += n;
        self.undo.extend(undos);
        self.items = kept;
        self.selected = 0;
        self.status = format!("applied {n} remaining proposals");
        Ok(())
    }

    fn undo_last(&mut self) {
        let Some(Undo { item, action }) = self.undo.pop() else {
            self.status = "nothing to undo".into();
            return;
        };
        // In-session undo restores the queue row. Live mailbox undo is not
        // implemented yet (would need move back from Archive).
        let _ = action;
        self.items.insert(self.selected.min(self.items.len()), item);
        self.applied = self.applied.saturating_sub(1);
        self.status = "undo (queue only)".into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mailbox::Demo;
    use crate::strategy::classify;
    use crate::types::Mode;

    #[test]
    fn enter_applies_proposals_leaves_unclaimed() {
        let mail = Demo::fixtures().envelopes;
        let items = classify(mail, Mode::Strategic, None);
        let mut app = App::new(items, Mode::Strategic);
        let before = app.items.len();
        let unclaimed = app.items.iter().filter(|i| i.proposal.is_none()).count();
        app.handle_key(KeyCode::Enter, &Demo::fixtures()).unwrap();
        assert_eq!(app.items.len(), unclaimed);
        assert!(app.applied > 0);
        assert!(app.applied < before);
    }

    #[test]
    fn skip_drops_without_counting_as_archive() {
        let mail = Demo::fixtures().envelopes;
        let items = classify(mail, Mode::Strategic, None);
        let mut app = App::new(items, Mode::Strategic);
        app.handle_key(KeyCode::Char('s'), &Demo::fixtures())
            .unwrap();
        assert_eq!(app.undo.last().unwrap().action, Action::Skip);
    }
}
