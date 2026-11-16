use super::Ui;
use crate::{client::Metrics, config::Settings};

pub(super) fn names(metrics: &Metrics, filter: &str) -> Vec<String> {
  let (exclude, filter) = filter
    .strip_prefix("exclude=")
    .map_or((false, filter), |filter| (true, filter));
  metrics
    .pools()
    .keys()
    .filter(|name| filter.trim().is_empty() || (filter.split_whitespace().any(|part| name.contains(part)) != exclude))
    .cloned()
    .collect()
}

pub(super) fn cycle(ui: &mut Ui, settings: &Settings, backwards: bool) {
  let names = ui
    .sessions
    .get(ui.selected)
    .and_then(Option::as_ref)
    .and_then(|session| {
      session
        .receiver
        .borrow()
        .history()
        .back()
        .map(|metrics| names(metrics, settings.disks_filter()))
    })
    .unwrap_or_default();
  if names.is_empty() {
    ui.pool = None;
    return;
  }
  let index = ui
    .pool
    .as_ref()
    .and_then(|selected| names.iter().position(|name| name == selected));
  let next = index.map_or(0, |index| {
    (index + if backwards { names.len() - 1 } else { 1 }) % names.len()
  });
  ui.pool = Some(names[next].clone());
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn it_filters_pools_cycles_both_ways_and_handles_missing_selections() {
    let state = crate::client::fixtures::state();
    let metrics = state.history().back().unwrap();
    assert_eq!(names(metrics, "storage"), vec!["storage_share"]);
    assert_eq!(names(metrics, "exclude=boot"), vec!["storage_share"]);
    assert_eq!(names(metrics, "boot storage"), vec!["boot-pool", "storage_share"]);
    assert!(names(metrics, "missing").is_empty());
    let (_, receiver) = tokio::sync::watch::channel(state);
    let mut ui = Ui::default();
    ui.sessions.push(Some(super::super::Session {
      interval: tokio::sync::watch::channel(2000).0,
      receiver,
      task: tokio::spawn(std::future::pending()),
    }));
    let settings = Settings::default();
    for (backwards, expected) in [(false, "boot-pool"), (true, "storage_share"), (false, "boot-pool")] {
      cycle(&mut ui, &settings, backwards);
      assert_eq!(ui.pool.as_deref(), Some(expected));
    }
    ui.pool = Some("removed".into());
    cycle(&mut ui, &settings, false);
    assert_eq!(ui.pool.as_deref(), Some("boot-pool"));
    cycle(
      &mut ui,
      &settings
        .with_preference("disks_filter", serde_json::json!("missing"))
        .unwrap(),
      false,
    );
    assert!(ui.pool.is_none());
  }
}
