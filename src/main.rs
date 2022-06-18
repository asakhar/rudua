use application::App;
/// A simple example demonstrating how to handle user input. This is
/// a bit out of the scope of the library as it does not provide any
/// input handling out of the box. However, it may helps some to get
/// started.
///
/// This is a very simple example:
///   * A input box always focused. Every character you type is registered
///   here
///   * Pressing Backspace erases a character
///   * Pressing Enter pushes the current input in the history of previous
///   messages
use crossterm::{
  event::{DisableMouseCapture, EnableMouseCapture},
  execute,
  terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{env::current_dir, error::Error, io, path::PathBuf};
use tui::{backend::CrosstermBackend, Terminal};

mod application;
mod utility;
use utility::{inspect_dir, Callbacks};

fn main() -> Result<(), Box<dyn Error>> {
  let dir = std::env::args()
    .nth(1)
    .map(|path| PathBuf::from(path))
    .unwrap_or(current_dir().expect("Failed to get current dir"));
  let mut total_used = fs2::total_space(&dir)? - fs2::free_space(&dir)?;
  let mut total_indexed = 0;
  const PROGRESS_WIDTH: usize = 40;

  let mut callbacks = Callbacks {
    failed_meta: &|error, path| {
      eprintln!(
        "Failed to get metadata of file {:?} with error {}",
        path, error
      );
    },
    failed_entry: &|error, parent| {
      eprintln!(
        "Failed to resolve directory entry. Parent directory {:?} with error {}",
        parent, error
      );
    },
    failed_inspections: &|error, parent| {
      eprintln!(
        "Failed to inspect directory {:?} with error {}",
        parent, error
      );
    },
    indexed_bytes: &mut |indexed_bytes| {
      total_indexed += indexed_bytes;
      if total_indexed > total_used {
        total_used += total_indexed
      }
      let percentage = total_indexed * 100 / total_used;
      let done: usize = total_indexed as usize * PROGRESS_WIDTH / total_used as usize;
      let undone: usize = PROGRESS_WIDTH - (done as usize) % PROGRESS_WIDTH;
      print!(
        "[ {}{} ] {}%     \r",
        "\u{2588}".repeat(done),
        " ".repeat(undone),
        percentage
      )
    },
  };
  let listing = inspect_dir(&dir, &mut callbacks).expect("Failed inspection");
  println!("Done!{}", " ".repeat(PROGRESS_WIDTH + 10));

  // setup terminal
  enable_raw_mode()?;
  let mut stdout = io::stdout();
  execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
  let backend = CrosstermBackend::new(stdout);
  let mut terminal = Terminal::new(backend)?;

  // create app and run it
  let mut app = App::new(listing);
  let res = app.run(&mut terminal);

  // restore terminal
  disable_raw_mode()?;
  execute!(
    terminal.backend_mut(),
    LeaveAlternateScreen,
    DisableMouseCapture
  )?;
  terminal.show_cursor()?;

  if let Err(err) = res {
    eprintln!("{:?}", err)
  }

  Ok(())
}
