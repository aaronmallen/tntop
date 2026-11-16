use ratatui::crossterm::event::KeyCode;

use super::{Ui, apply_preferences};
use crate::{
  client::{Application, State},
  config::Settings,
};

pub(super) fn ordered<'a>(state: &'a State, filter: &str, settings: &Settings) -> Vec<&'a Application> {
  let stats = state.app_history().back();
  let filter = filter.to_lowercase();
  let mut apps: Vec<_> = state
    .apps()
    .iter()
    .flatten()
    .filter(|app| app.name().to_lowercase().contains(&filter))
    .collect();
  apps.sort_by(|a, b| {
    let a_stats = stats.and_then(|stats| stats.get(a.name()));
    let b_stats = stats.and_then(|stats| stats.get(b.name()));
    let missing = if matches!(settings.apps_sort().as_str(), "cpu" | "memory") {
      a_stats.is_none().cmp(&b_stats.is_none())
    } else {
      std::cmp::Ordering::Equal
    };
    let order = match settings.apps_sort().as_str() {
      "cpu" => b_stats
        .map(|stats| *stats.cpu_usage())
        .partial_cmp(&a_stats.map(|stats| *stats.cpu_usage()))
        .unwrap_or(std::cmp::Ordering::Equal),
      "memory" => b_stats
        .map(|stats| *stats.memory())
        .cmp(&a_stats.map(|stats| *stats.memory())),
      "state" => a.state().cmp(b.state()),
      _ => a.name().cmp(b.name()),
    };
    missing
      .then(if settings.apps_reversed() {
        order.reverse()
      } else {
        order
      })
      .then_with(|| a.name().cmp(b.name()))
  });
  apps
}

pub(super) fn handle(ui: &mut Ui, settings: &mut Settings, path: &std::path::Path, key: KeyCode) -> bool {
  let (key, value) = match key {
    KeyCode::Left | KeyCode::Right => {
      let choices = ["cpu", "memory", "name", "state"];
      let index = choices
        .iter()
        .position(|choice| *choice == settings.apps_sort())
        .unwrap_or(0);
      let index = (index + if key == KeyCode::Right { 1 } else { choices.len() - 1 }) % choices.len();
      ("apps_sort", serde_json::json!(choices[index]))
    }
    KeyCode::Char('r') => ("apps_reversed", serde_json::json!(!settings.apps_reversed())),
    _ => return false,
  };
  let result = settings
    .with_preference(key, value)
    .map_err(super::Error::from)
    .and_then(|candidate| apply_preferences(ui, settings, path, candidate));
  if let Err(error) = result {
    ui.message = Some(error.to_string());
  }
  true
}

pub(super) fn cpu(value: f64, state: &State, settings: &Settings) -> Option<f64> {
  if !settings.apps_per_core() {
    return Some(value);
  }
  let cores = state
    .history()
    .back()?
    .cpu()
    .keys()
    .filter(|name| name.as_str() != "cpu")
    .count();
  (cores > 0).then_some(value * cores as f64)
}

#[cfg(test)]
mod tests {
  use serde_json::json;

  use super::*;

  #[test]
  fn it_sorts_reverses_breaks_ties_and_normalizes_cpu_only_with_known_cores() {
    let state = crate::client::fixtures::state();
    let mut settings = Settings::default()
      .with_preference("apps_sort", json!("memory"))
      .unwrap();
    assert_eq!(ordered(&state, "", &settings)[0].name(), "forgejo");
    settings = settings.with_preference("apps_reversed", json!(true)).unwrap();
    assert_eq!(ordered(&state, "", &settings)[0].name(), "jellyfin");
    let missing = crate::client::fixtures::without_app_stats();
    let names: Vec<_> = ordered(&missing, "", &settings)
      .iter()
      .map(|app| app.name().as_str())
      .collect();
    assert_eq!(names, ["forgejo", "jellyfin", "postgres"]);
    assert_eq!(ordered(&state, "POST", &settings)[0].name(), "postgres");
    assert_eq!(cpu(25.0, &state, &settings), Some(25.0));
    settings = settings.with_preference("apps_per_core", json!(true)).unwrap();
    assert_eq!(cpu(25.0, &state, &settings), Some(100.0));
    assert_eq!(cpu(25.0, &State::default(), &settings), None);
  }
}
