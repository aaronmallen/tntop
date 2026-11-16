use std::path::Path;

use ratatui::{
  Frame,
  crossterm::event::KeyCode,
  layout::Rect,
  text::Line,
  widgets::{Clear, Paragraph, Wrap},
};
use zeroize::Zeroizing;

use super::{Error, Session, Ui, view::panel};
use crate::{
  client,
  config::{self, Server, Settings},
  credentials,
};

pub(super) enum Dialog {
  Add { field: usize, values: [String; 3] },
  Delete,
  Key { value: Zeroizing<String> },
  Trust { fingerprint: String },
}

pub(super) fn discover(ui: &mut Ui, settings: &Settings) {
  let server = settings.servers()[ui.selected].clone();
  if server.url().scheme() != "https" {
    ui.message = Some("HTTP connections do not use certificates.".into());
    return;
  }
  ui.message = None;
  ui.picker = true;
  ui.discovery = Some(tokio::spawn(async move { client::tls::discover(&server).await }));
}

pub(super) fn draw(frame: &mut Frame, ui: &Ui, settings: &Settings) {
  let server = settings.servers().get(ui.selected);
  let (title, mut lines) = match &ui.dialog {
    Some(Dialog::Add {
      field,
      values,
    }) => (
      " add server ",
      vec![
        "Tab / ↑↓ switch fields. Enter saves. Esc cancels.".into(),
        String::new(),
        format!("{} Name: {}", if *field == 0 { "›" } else { " " }, values[0]),
        format!("{} HTTP(S) URL: {}", if *field == 1 { "›" } else { " " }, values[1]),
        format!(
          "{} CA PEM path (optional): {}",
          if *field == 2 { "›" } else { " " },
          values[2]
        ),
        String::new(),
        "For HTTPS, leave CA path empty to trust a certificate fingerprint.".into(),
      ],
    ),
    Some(Dialog::Delete) => (
      " delete server ",
      vec![
        format!("Delete {} from tntop?", server.unwrap().name()),
        "Stops monitoring and removes its saved trust.".into(),
        "Removes its API key unless another entry uses the same URL.".into(),
        "The NAS itself is not changed.".into(),
        String::new(),
        "y delete / n or Esc cancel".into(),
      ],
    ),
    Some(Dialog::Key {
      value,
    }) => (
      " API key ",
      vec![
        format!("API key for {}", server.unwrap().name()),
        "Saves immediately to your system keyring.".into(),
        String::new(),
        "•".repeat(value.chars().count()),
        String::new(),
        "Enter save / Esc cancel".into(),
      ],
    ),
    Some(Dialog::Trust {
      fingerprint,
    }) => (
      " trust certificate ",
      vec![
        format!("{} ({})", server.unwrap().name(), server.unwrap().url()),
        "Presented certificate SHA-256:".into(),
        fingerprint.clone(),
        format!(
          "Saved: {}",
          server.unwrap().certificate_sha256().as_deref().unwrap_or("none")
        ),
        "Compare with your NAS through a trusted connection.".into(),
        "Approval replaces saved trust, including any CA setting.".into(),
        String::new(),
        "y trust / n or Esc cancel".into(),
      ],
    ),
    None if ui.discovery.is_some() => (
      " certificate ",
      vec!["Reading the server certificate…".into(), "Esc cancels.".into()],
    ),
    None => return,
  };
  if let Some(error) = &ui.message {
    lines.push(String::new());
    lines.push(error.clone());
  }
  let area = frame.area();
  let width = area.width.saturating_sub(4).min(90);
  let height = area.height.saturating_sub(2).min(17);
  let area = Rect::new(
    area.x + (area.width - width) / 2,
    area.y + (area.height - height) / 2,
    width,
    height,
  );
  frame.render_widget(Clear, area);
  frame.render_widget(
    Paragraph::new(lines.into_iter().map(Line::from).collect::<Vec<_>>())
      .style(ui.theme.style())
      .block(panel(title, "cpu_box", &ui.theme))
      .wrap(Wrap {
        trim: false,
      }),
    area,
  );
}

pub(super) async fn handle(ui: &mut Ui, settings: &mut Settings, path: &Path, key: KeyCode) -> Result<(), Error> {
  if key == KeyCode::Esc {
    ui.dialog = None;
    ui.message = None;
    return Ok(());
  }
  match ui.dialog.as_mut() {
    Some(Dialog::Add {
      field,
      values,
    }) => match key {
      KeyCode::Tab | KeyCode::Down => *field = (*field + 1) % values.len(),
      KeyCode::BackTab | KeyCode::Up => *field = (*field + values.len() - 1) % values.len(),
      KeyCode::Backspace => {
        values[*field].pop();
      }
      KeyCode::Char(c) if !c.is_control() => values[*field].push(c),
      KeyCode::Enter => {
        let mut server = Server::new(
          values[0].trim().into(),
          values[1]
            .trim()
            .parse()
            .map_err(|_| config::Error::Invalid("enter an HTTP or HTTPS server URL".into()))?,
        )?;
        if settings
          .servers()
          .iter()
          .any(|existing| existing.name() == server.name())
        {
          return Err(config::Error::Invalid(format!("server already exists: {}", server.name())).into());
        }
        if !values[2].trim().is_empty() {
          server = server.with_ca_cert(std::fs::canonicalize(values[2].trim())?);
        }
        config::add_server(path, &server)?;
        settings.servers_mut().push(server);
        ui.sessions.push(None);
        ui.selected = settings.servers().len() - 1;
        ui.dialog = None;
        open(ui, settings).await?;
      }
      _ => {}
    },
    Some(Dialog::Delete) => match key {
      KeyCode::Char('y') => {
        let server = settings.servers()[ui.selected].clone();
        config::remove_server(path, &server)?;
        settings.servers_mut().remove(ui.selected);
        ui.sessions.remove(ui.selected);
        ui.selected = ui.selected.min(settings.servers().len().saturating_sub(1));
        ui.dialog = None;
        ui.message = Some(format!("Removed {}", server.name()));
        if !settings.servers().iter().any(|other| other.url() == server.url())
          && let Err(error) = credentials::delete(&server).await
        {
          ui.message = Some(format!("Server removed; could not remove its saved API key: {error}"));
        }
      }
      KeyCode::Char('n') => ui.dialog = None,
      _ => {}
    },
    Some(Dialog::Key {
      value,
    }) => match key {
      KeyCode::Backspace => {
        value.pop();
      }
      KeyCode::Char(c) if !c.is_control() => value.push(c),
      KeyCode::Enter => {
        credentials::save(&settings.servers()[ui.selected], value.clone()).await?;
        ui.sessions[ui.selected] = None;
        ui.dialog = None;
        open(ui, settings).await?;
      }
      _ => {}
    },
    Some(Dialog::Trust {
      fingerprint,
    }) => match key {
      KeyCode::Char('y') => {
        let server = &mut settings.servers_mut()[ui.selected];
        config::pin_certificate(path, server, fingerprint)?;
        *server = server.clone().with_certificate_sha256(fingerprint.clone());
        ui.sessions[ui.selected] = None;
        ui.dialog = None;
        open(ui, settings).await?;
      }
      KeyCode::Char('n') => ui.dialog = None,
      _ => {}
    },
    None => {}
  }
  Ok(())
}

pub(super) async fn open(ui: &mut Ui, settings: &Settings) -> Result<(), Error> {
  let Some(server) = settings.servers().get(ui.selected) else {
    return Ok(());
  };
  if ui.sessions[ui.selected].is_none() {
    ui.picker = true;
    let Some(key) = credentials::load(server).await? else {
      ui.dialog = Some(Dialog::Key {
        value: Default::default(),
      });
      return Ok(());
    };
    if server.url().scheme() == "https" && server.ca_cert().is_none() && server.certificate_sha256().is_none() {
      discover(ui, settings);
      return Ok(());
    }
    let (receiver, task) = client::watch(server.clone(), key, settings.update_ms());
    ui.sessions[ui.selected] = Some(Session {
      receiver,
      task,
    });
  }
  ui.app_row = 0;
  ui.interface = 0;
  ui.pool = 0;
  ui.filter.clear();
  ui.message = None;
  ui.picker = false;
  Ok(())
}

#[cfg(test)]
mod tests {
  use ratatui::{Terminal, backend::TestBackend};

  use super::*;

  fn ui() -> Ui {
    Ui {
      app_row: 0,
      dialog: None,
      discovery: None,
      filter: String::new(),
      filtering: false,
      interface: 0,
      message: None,
      picker: true,
      pool: 0,
      selected: 0,
      sessions: vec![],
      sort: 0,
      theme: Default::default(),
    }
  }

  mod discover {
    use super::*;

    #[test]
    fn it_skips_certificate_discovery_for_http() {
      let mut settings = Settings::default();
      settings
        .servers_mut()
        .push(Server::new("home".into(), "http://nas.local".parse().unwrap()).unwrap());
      let mut ui = ui();
      discover(&mut ui, &settings);
      assert!(ui.discovery.is_none());
      assert!(ui.dialog.is_none());
    }
  }

  mod draw {
    use super::*;

    #[test]
    fn it_masks_api_keys_and_renders_each_manager_dialog() {
      let mut settings = Settings::default();
      settings
        .servers_mut()
        .push(Server::new("home".into(), "https://nas.local".parse().unwrap()).unwrap());
      let mut ui = ui();
      for dialog in [
        Dialog::Add {
          field: 1,
          values: ["home".into(), "https://nas.local".into(), String::new()],
        },
        Dialog::Delete,
        Dialog::Key {
          value: Zeroizing::new("never-show-this-secret".into()),
        },
        Dialog::Trust {
          fingerprint: "ab".repeat(32),
        },
      ] {
        ui.dialog = Some(dialog);
        let mut terminal = Terminal::new(TestBackend::new(100, 32)).unwrap();
        terminal
          .draw(|frame| super::super::draw(frame, &ui, &settings))
          .unwrap();
        let text: String = terminal
          .backend()
          .buffer()
          .content
          .iter()
          .map(|cell| cell.symbol())
          .collect();
        assert!(text.contains("home"));
        assert!(!text.contains("never-show-this-secret"));
        if matches!(ui.dialog, Some(Dialog::Key { .. })) {
          assert!(text.contains("••••"));
        }
      }
    }
  }

  mod handle {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_cancels_trust_without_changing_the_saved_fingerprint() {
      let directory = tempfile::tempdir().unwrap();
      let path = directory.path().join("config.toml");
      let server = Server::new("home".into(), "https://nas.local".parse().unwrap()).unwrap();
      config::add_server(&path, &server).unwrap();
      config::pin_certificate(&path, &server, &"ab".repeat(32)).unwrap();
      let mut settings = config::load(&path).unwrap();
      let before = std::fs::read_to_string(&path).unwrap();
      let mut ui = ui();
      ui.dialog = Some(Dialog::Trust {
        fingerprint: "cd".repeat(32),
      });
      super::super::handle(&mut ui, &mut settings, &path, KeyCode::Char('n'))
        .await
        .unwrap();
      assert!(ui.dialog.is_none());
      assert_eq!(std::fs::read_to_string(path).unwrap(), before);
    }

    #[tokio::test]
    async fn it_deletes_a_shared_url_entry_without_removing_the_other_entry_or_key() {
      let directory = tempfile::tempdir().unwrap();
      let path = directory.path().join("config.toml");
      for name in ["first", "second"] {
        config::add_server(
          &path,
          &Server::new(name.into(), "https://nas.local".parse().unwrap()).unwrap(),
        )
        .unwrap();
      }
      let mut settings = config::load(&path).unwrap();
      let mut ui = ui();
      ui.selected = 1;
      ui.sessions = vec![None, None];
      ui.dialog = Some(Dialog::Delete);
      super::super::handle(&mut ui, &mut settings, &path, KeyCode::Char('y'))
        .await
        .unwrap();
      assert_eq!(ui.selected, 0);
      assert_eq!(ui.sessions.len(), 1);
      assert_eq!(settings.servers()[0].name(), "first");
      assert_eq!(config::load(&path).unwrap().servers().len(), 1);
    }

    #[tokio::test]
    async fn it_keeps_an_invalid_add_form_for_correction_and_supports_cancel() {
      let directory = tempfile::tempdir().unwrap();
      let path = directory.path().join("config.toml");
      let mut settings = Settings::default();
      let mut ui = ui();
      ui.dialog = Some(Dialog::Add {
        field: 0,
        values: ["home".into(), "ftp://nas.local".into(), String::new()],
      });
      assert!(
        super::super::handle(&mut ui, &mut settings, &path, KeyCode::Enter)
          .await
          .is_err()
      );
      assert!(ui.dialog.is_some());
      assert!(!path.exists());
      super::super::handle(&mut ui, &mut settings, &path, KeyCode::Tab)
        .await
        .unwrap();
      assert!(matches!(
        ui.dialog,
        Some(Dialog::Add {
          field: 1,
          ..
        })
      ));
      super::super::handle(&mut ui, &mut settings, &path, KeyCode::Esc)
        .await
        .unwrap();
      assert!(ui.dialog.is_none());
      assert!(settings.servers().is_empty());
    }
  }
}
