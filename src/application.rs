use crossterm::event::{self, Event, KeyCode};
use std::{cmp::max, io, path::PathBuf};
use tui::{
  backend::Backend,
  layout::{Constraint, Direction, Layout},
  style::{Color, Modifier, Style},
  text::{Span, Spans, Text},
  widgets::{Block, Borders, List, ListItem, Paragraph},
  Frame, Terminal,
};
use unicode_width::UnicodeWidthStr;

use crate::utility::Node;

pub enum Mode {
  Normal,
  AskRemove,
}

pub struct App {
  mode: Mode,
  listing: Node,
  current: PathBuf,
  selected: usize,
  number_of_files: usize,
}

impl App {
  pub fn new(node: Node) -> Self {
    let name = node.name.clone();
    let number_of_files = node.children.len();
    Self {
      mode: Mode::Normal,
      listing: node,
      current: PathBuf::from(name),
      selected: 0,
      number_of_files,
    }
  }
  pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
    loop {
      terminal.draw(|f| self.ui(f))?;
      match self.mode {
        Mode::Normal => {
          if let Event::Key(key) = event::read()? {
            match key.code {
              KeyCode::Char('q') | KeyCode::Esc => {
                return Ok(());
              }
              KeyCode::Down => {
                self.selected += (self.selected < self.number_of_files - 1) as usize;
              }
              KeyCode::Up => {
                self.selected -= (self.selected > 0) as usize;
              }
              KeyCode::Enter => {
                let current = self.listing.find_node(&self.current).expect("Invalid path");
                let current = current
                  .children
                  .iter()
                  .nth(self.selected)
                  .expect("Invalid index");
                if !current.is_directory {
                  continue;
                }
                self.current = current.name.clone();
                self.selected = 0;
                self.number_of_files = current.children.len();
              }
              KeyCode::Char(' ') => {
                let current = self
                  .listing
                  .find_node_mut(&self.current)
                  .expect("Invalid path");
                let current = current
                  .children
                  .iter_mut()
                  .nth(self.selected)
                  .expect("Invalid index");
                current.mark(!current.is_marked);
              }
              KeyCode::Delete => {
                self.mode = Mode::AskRemove;
              }
              KeyCode::Backspace => {
                let parent = if let Some(ancestor) = self.current.ancestors().nth(1) {
                  ancestor
                } else {
                  continue;
                };
                let current = if let Some(path) = self.listing.find_node(parent) {
                  path
                } else {
                  continue;
                };
                self.current = current.name.clone();
                self.selected = 0;
                self.number_of_files = current.children.len();
              }
              _ => {}
            }
          }
        }
        Mode::AskRemove => {
          if let Event::Key(key) = event::read()? {
            match key.code {
              KeyCode::Char('y') => {
                self.listing.delete_marked();
                return Ok(());
              }
              KeyCode::Char('n') => {
                self.mode = Mode::Normal;
              }
              _ => {}
            }
          }
        }
      }
    }
  }

  pub fn ui<B: Backend>(&self, f: &mut Frame<B>) {
    match self.mode {
      Mode::Normal => {
        let chunks = Layout::default()
          .direction(Direction::Vertical)
          .margin(1)
          .constraints([Constraint::Length(2), Constraint::Min(1)].as_ref())
          .split(f.size());

        let msg = vec![
          Span::raw("Navigate with arrow keys. Press "),
          Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
          Span::raw(" to enter selected directory, "),
          Span::styled("Backspace", Style::default().add_modifier(Modifier::BOLD)),
          Span::raw(" to return to parent directory."),
        ];
        let text = Text::from(Spans::from(msg));
        let help_message = Paragraph::new(text);
        f.render_widget(help_message, chunks[0]);
        let current_node = self.listing.find_node(&self.current).expect("Invalid path");
        let listing = current_node.children.iter().enumerate().map(|(i, child)| {
          (
            i,
            child
              .name
              .file_name()
              .expect("Cannot end with ..")
              .to_string_lossy()
              .into_owned(),
            child.size,
            child.is_marked,
          )
        });
        let height = f.size().height as usize - 6;
        let listing = listing.skip(self.selected / height * height);
        let max_width = listing
          .clone()
          .map(|(_, child, _, _)| UnicodeWidthStr::width(child.as_str()))
          .reduce(|acc, file_size| max(file_size, acc))
          .unwrap_or(0);
        let mut listing: Vec<ListItem> = listing
          .map(|(i, child, file_size, marked)| {
            let width = UnicodeWidthStr::width(child.as_str());
            let text = format!("{}{}   {}", child, " ".repeat(max_width - width), file_size);
            let span = if i == self.selected {
              Span::styled(
                text,
                Style::default()
                  .bg(if marked { Color::Cyan } else { Color::Gray })
                  .fg(if marked { Color::Red } else { Color::Black }),
              )
            } else {
              Span::styled(
                text,
                Style::default().fg(if marked { Color::Magenta } else { Color::White }),
              )
            };
            ListItem::new(span)
          })
          .collect();
        if listing.len() == 0 {
          listing.push(ListItem::new(Span::styled(
            "$ Empty directory $",
            Style::default()
              .add_modifier(Modifier::BOLD)
              .fg(Color::Yellow),
          )));
        }

        let dir_listing = List::new(listing).block(
          Block::default()
            .borders(Borders::ALL)
            .title("Files and folders"),
        );
        f.render_widget(dir_listing, chunks[1]);
      }
      Mode::AskRemove => {
        let chunks = Layout::default()
          .direction(Direction::Vertical)
          .margin(2)
          .constraints([Constraint::Length(2), Constraint::Min(1)].as_ref())
          .split(f.size());

        let msg = vec![
          Span::raw("Do you "),
          Span::styled("REALLY", Style::default().add_modifier(Modifier::BOLD)),
          Span::raw(" want to "),
          Span::styled(
            "REMOVE MARKED FILES",
            Style::default().add_modifier(Modifier::BOLD),
          ),
          Span::raw("?"),
        ];
        let mut text = Text::from(Spans::from(msg));
        text.patch_style(Style::default().fg(Color::Red));
        let help_message = Paragraph::new(text);
        f.render_widget(help_message, chunks[0]);

        let msg = vec![
          Span::raw("Press "),
          Span::styled("Y", Style::default().add_modifier(Modifier::BOLD)),
          Span::raw(" to remove selected files and "),
          Span::styled("N", Style::default().add_modifier(Modifier::BOLD)),
          Span::raw(" if you changed your mind."),
        ];
        let question = Paragraph::new(Text::from(Spans::from(msg)));
        f.render_widget(question, chunks[1]);
      }
    }
  }
}
