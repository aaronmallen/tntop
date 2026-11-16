use ratatui::crossterm::event::KeyCode;

use super::{Ui, apply_preferences};
use crate::config::Settings;

pub(super) fn handle(ui: &mut Ui, settings: &mut Settings, path: &std::path::Path, key: KeyCode) -> bool {
  let reverse = match key {
    KeyCode::Char('p') => false,
    KeyCode::Char('P') => true,
    _ => return false,
  };
  let result = (|| {
    let available: Vec<_> = (0..settings.presets()?.len())
      .filter(|index| settings.disable_presets().permits(*index))
      .collect();
    let selected = if reverse {
      available
        .iter()
        .rev()
        .find(|index| ui.preset.is_some_and(|current| **index < current))
        .or(available.last())
    } else {
      available
        .iter()
        .find(|index| ui.preset.is_some_and(|current| **index > current))
        .or(available.first())
    }
    .copied()
    .ok_or_else(|| crate::config::Error::Invalid("All available presets are disabled.".into()))?;
    let candidate = settings.with_preset(selected)?;
    apply_preferences(ui, settings, path, candidate)?;
    ui.preset = settings
      .presets()?
      .get(selected)
      .filter(|preset| preset.same_layout(settings))
      .map(|_| selected);
    Ok::<_, super::Error>(())
  })();
  ui.message = result.err().map(|error| error.to_string());
  true
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn it_wraps_skips_disabled_presets_and_keeps_sessions_after_manual_edits() {
    let mut settings = Settings::default();
    let mut ui = Ui::default();
    let (_, receiver) = tokio::sync::watch::channel(crate::client::fixtures::state());
    let task = tokio::spawn(std::future::pending());
    let id = task.id();
    ui.sessions.push(Some(super::super::Session {
      receiver,
      task,
    }));
    let path = std::path::Path::new("config.toml");
    for expected in [0, 1, 2, 3, 0] {
      handle(&mut ui, &mut settings, path, KeyCode::Char('p'));
      assert_eq!(ui.preset, Some(expected));
      assert!(settings.same_layout(&settings.presets().unwrap()[expected]));
    }
    handle(&mut ui, &mut settings, path, KeyCode::Char('P'));
    assert_eq!(ui.preset, Some(3));
    for (policy, expected) in [("default", Some(1)), ("custom", Some(0)), ("all", Some(0))] {
      let candidate = settings
        .with_preference("disable_presets", serde_json::json!(policy))
        .unwrap();
      apply_preferences(&mut ui, &mut settings, path, candidate).unwrap();
      handle(&mut ui, &mut settings, path, KeyCode::Char('p'));
      assert_eq!(ui.preset, expected);
      assert_eq!(ui.message.is_some(), policy == "all");
    }
    super::super::layout::handle(&mut ui, &mut settings, path, KeyCode::Char('1'));
    assert_eq!(ui.preset, None);
    assert_eq!(ui.sessions[0].as_ref().unwrap().task.id(), id);
    assert!(!ui.sessions[0].as_ref().unwrap().receiver.borrow().history().is_empty());
    let candidate = settings
      .with_preference("presets", serde_json::json!(""))
      .unwrap()
      .with_preference("disable_presets", serde_json::json!("default"))
      .unwrap();
    apply_preferences(&mut ui, &mut settings, path, candidate).unwrap();
    handle(&mut ui, &mut settings, path, KeyCode::Char('P'));
    assert!(ui.message.as_ref().unwrap().contains("disabled"));
    assert_eq!(ui.preset, None);
  }

  #[test]
  fn it_cycles_duplicate_layouts_without_getting_stuck() {
    let mut settings = Settings::default()
      .with_preference("presets", serde_json::json!("cpu:0:default cpu:0:default"))
      .unwrap();
    let mut ui = Ui::default();
    for expected in [0, 1, 2, 0] {
      handle(
        &mut ui,
        &mut settings,
        std::path::Path::new("config.toml"),
        KeyCode::Char('p'),
      );
      assert_eq!(ui.preset, Some(expected));
    }
  }
}
