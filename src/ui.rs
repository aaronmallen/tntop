mod manager;
mod theme;
mod view;

use std::{
  io::IsTerminal,
  path::Path,
  time::{Duration, Instant},
};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use theme::Theme;
use tokio::sync::watch;

use crate::{client, config::Settings};

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error(transparent)]
  Client(#[from] client::Error),
  #[error(transparent)]
  Configuration(#[from] crate::config::Error),
  #[error(transparent)]
  Credentials(#[from] crate::credentials::Error),
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[error("tntop requires an interactive terminal")]
  NoTerminal,
  #[error(transparent)]
  Theme(#[from] theme::Error),
  #[error("no configured server named {0}")]
  UnknownServer(String),
}

struct Session {
  receiver: watch::Receiver<client::State>,
  task: tokio::task::JoinHandle<()>,
}

impl Drop for Session {
  fn drop(&mut self) {
    self.task.abort();
  }
}

struct Ui {
  app_row: usize,
  dialog: Option<manager::Dialog>,
  discovery: Option<tokio::task::JoinHandle<Result<String, client::Error>>>,
  filter: String,
  filtering: bool,
  interface: usize,
  message: Option<String>,
  picker: bool,
  pool: usize,
  selected: usize,
  sessions: Vec<Option<Session>>,
  sort: usize,
  theme: Theme,
}

impl Drop for Ui {
  fn drop(&mut self) {
    if let Some(task) = &self.discovery {
      task.abort();
    }
  }
}

pub async fn run(settings: &Settings, config_path: &Path, server: Option<&str>) -> Result<(), Error> {
  let mut settings = settings.clone();
  let selected = select(&settings, server)?;
  if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
    return Err(Error::NoTerminal);
  }
  let theme_path = settings.theme().as_ref().map(|path| {
    if path.is_absolute() {
      path.clone()
    } else {
      config_path.parent().unwrap_or(Path::new(".")).join(path)
    }
  });
  let mut ui = Ui {
    app_row: 0,
    dialog: None,
    discovery: None,
    filter: String::new(),
    filtering: false,
    interface: 0,
    message: None,
    picker: selected.is_none(),
    pool: 0,
    selected: selected.unwrap_or(0),
    sessions: (0..settings.servers().len()).map(|_| None).collect(),
    sort: 0,
    theme: Theme::load(theme_path.as_deref())?,
  };
  let level = log::max_level();
  log::set_max_level(log::LevelFilter::Off);
  let result = run_terminal(&mut ui, &mut settings, config_path).await;
  ratatui::restore();
  log::set_max_level(level);
  result
}

async fn run_terminal(ui: &mut Ui, settings: &mut Settings, config_path: &Path) -> Result<(), Error> {
  let mut terminal = ratatui::try_init()?;
  if !ui.picker {
    terminal.draw(|frame| view::draw(frame, ui, settings))?;
    if let Err(error) = manager::open(ui, settings).await {
      ui.picker = true;
      ui.message = Some(error.to_string());
    }
  }
  let mut redraw = Instant::now();
  loop {
    if ui.discovery.as_ref().is_some_and(|task| task.is_finished()) {
      let result = ui.discovery.take().unwrap().await;
      match result {
        Ok(Ok(fingerprint)) => {
          ui.dialog = Some(manager::Dialog::Trust {
            fingerprint,
          })
        }
        Ok(Err(error)) => ui.message = Some(error.to_string()),
        Err(error) => ui.message = Some(error.to_string()),
      }
      redraw = Instant::now();
    }
    if redraw <= Instant::now() {
      terminal.draw(|frame| view::draw(frame, ui, settings))?;
      redraw = Instant::now() + Duration::from_millis(settings.update_ms());
    }
    if event::poll(Duration::ZERO)? {
      let event = event::read()?;
      redraw = Instant::now();
      let Event::Key(key) = event else { continue };
      if key.kind == KeyEventKind::Release {
        continue;
      }
      if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Ok(());
      }
      if ui.discovery.is_some() {
        if key.code == KeyCode::Esc {
          ui.discovery.take().unwrap().abort();
          ui.message = None;
        }
        continue;
      }
      if ui.dialog.is_some() {
        if let Err(error) = manager::handle(ui, settings, config_path, key.code).await {
          ui.message = Some(error.to_string());
        }
        continue;
      }
      if ui.filtering {
        match key.code {
          KeyCode::Esc | KeyCode::Enter => ui.filtering = false,
          KeyCode::Backspace => {
            ui.filter.pop();
          }
          KeyCode::Char(c) => ui.filter.push(c),
          _ => {}
        }
        ui.app_row = 0;
        continue;
      }
      match key.code {
        KeyCode::Char('q') => return Ok(()),
        KeyCode::Esc if ui.picker => return Ok(()),
        KeyCode::Esc | KeyCode::Char('s') => ui.picker = true,
        KeyCode::Down | KeyCode::Char('j') if ui.picker => {
          ui.selected = (ui.selected + 1) % settings.servers().len().max(1)
        }
        KeyCode::Up | KeyCode::Char('k') if ui.picker => {
          ui.selected = (ui.selected + settings.servers().len().max(1) - 1) % settings.servers().len().max(1)
        }
        KeyCode::Enter if ui.picker => {
          let result = manager::open(ui, settings).await;
          if let Err(error) = result {
            ui.message = Some(error.to_string());
          }
        }
        KeyCode::Char('a') if ui.picker => {
          ui.message = None;
          ui.dialog = Some(manager::Dialog::Add {
            field: 0,
            values: Default::default(),
          });
        }
        KeyCode::Char('d') if ui.picker && !settings.servers().is_empty() => {
          ui.message = None;
          ui.dialog = Some(manager::Dialog::Delete);
        }
        KeyCode::Char('e') if ui.picker && !settings.servers().is_empty() => {
          ui.message = None;
          ui.dialog = Some(manager::Dialog::Key {
            value: Default::default(),
          });
        }
        KeyCode::Char('t') if ui.picker && !settings.servers().is_empty() => manager::discover(ui, settings),
        KeyCode::Down | KeyCode::Char('j') => ui.app_row = ui.app_row.saturating_add(1),
        KeyCode::Up | KeyCode::Char('k') => ui.app_row = ui.app_row.saturating_sub(1),
        KeyCode::Char('/') if !ui.picker => ui.filtering = true,
        KeyCode::Char('o') if !ui.picker => {
          ui.sort = (ui.sort + 1) % 4;
          ui.app_row = 0;
        }
        KeyCode::Char('n') if !ui.picker => ui.interface = ui.interface.wrapping_add(1),
        KeyCode::Char('p') if !ui.picker => ui.pool = ui.pool.wrapping_add(1),
        _ => {}
      }
    }
    tokio::time::sleep(Duration::from_millis(16)).await;
  }
}

fn select(settings: &Settings, name: Option<&str>) -> Result<Option<usize>, Error> {
  match name {
    Some(name) => settings
      .servers()
      .iter()
      .position(|server| server.name() == name)
      .map(Some)
      .ok_or_else(|| Error::UnknownServer(name.into())),
    None if settings.servers().len() == 1 => Ok(Some(0)),
    None => Ok(None),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod select {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_opens_only_a_single_or_explicitly_named_server() {
      let mut settings = Settings::default();
      assert_eq!(select(&settings, None).unwrap(), None);
      assert!(matches!(
        select(&settings, Some("missing")),
        Err(Error::UnknownServer(_))
      ));
      settings
        .servers_mut()
        .push(crate::config::Server::new("home".into(), "https://home.local".parse().unwrap()).unwrap());
      assert_eq!(select(&settings, None).unwrap(), Some(0));
      settings
        .servers_mut()
        .push(crate::config::Server::new("work".into(), "https://work.local".parse().unwrap()).unwrap());
      assert_eq!(select(&settings, None).unwrap(), None);
      assert_eq!(select(&settings, Some("work")).unwrap(), Some(1));
    }
  }
}
