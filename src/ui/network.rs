use ratatui::crossterm::event::KeyCode;

use super::{Ui, apply_preferences};
use crate::{client::State, config::Settings};

pub(super) fn names(state: &State) -> Vec<&str> {
  let Some(sample) = state.history().back() else {
    return Vec::new();
  };
  let mut names: Vec<_> = sample.interfaces().keys().map(String::as_str).collect();
  names.sort_by_key(|name| {
    (
      sample.interfaces()[*name].link_state() != "LINK_STATE_UP",
      *name == "lo",
      *name,
    )
  });
  names
}

pub(super) fn selected<'a>(state: &'a State, settings: &Settings) -> Option<&'a str> {
  let names = names(state);
  names
    .iter()
    .copied()
    .find(|name| *name == settings.net_iface())
    .or_else(|| {
      let sample = state.history().back()?;
      names.iter().copied().max_by(|a, b| {
        let total = |name: &str| {
          state
            .network_totals()
            .get(name)
            .map_or(0.0, |totals| totals.downloaded() + totals.uploaded())
        };
        (sample.interfaces()[*a].link_state() == "LINK_STATE_UP")
          .cmp(&(sample.interfaces()[*b].link_state() == "LINK_STATE_UP"))
          .then_with(|| total(a).total_cmp(&total(b)))
          .then_with(|| b.cmp(a))
      })
    })
}

pub(super) fn handle(ui: &mut Ui, settings: &mut Settings, path: &std::path::Path, key: KeyCode) -> bool {
  if !matches!(key, KeyCode::Char('a' | 'b' | 'n' | 'y' | 'z')) {
    return false;
  }
  let state = ui
    .sessions
    .get(ui.selected)
    .and_then(Option::as_ref)
    .map(|session| session.receiver.borrow().clone())
    .unwrap_or_default();
  let name = selected(&state, settings);
  let preference = match key {
    KeyCode::Char('a') => Some(("net_auto", serde_json::json!(!settings.net_auto()))),
    KeyCode::Char('y') => Some(("net_sync", serde_json::json!(!settings.net_sync()))),
    KeyCode::Char('z') => {
      if let Some(name) = name {
        let key = (ui.selected, name.to_owned());
        if ui.net_baselines.remove(&key).is_none()
          && let Some(totals) = state.network_totals().get(name)
        {
          ui.net_baselines.insert(key, (totals.downloaded(), totals.uploaded()));
        }
      }
      None
    }
    _ => {
      let names = names(&state);
      name.map(|name| {
        let index = names.iter().position(|candidate| *candidate == name).unwrap();
        let next = (index + if key == KeyCode::Char('b') { names.len() - 1 } else { 1 }) % names.len();
        ("net_iface", serde_json::json!(names[next]))
      })
    }
  };
  if let Some((key, value)) = preference {
    let result = settings
      .with_preference(key, value)
      .map_err(super::Error::from)
      .and_then(|candidate| apply_preferences(ui, settings, path, candidate));
    if let Err(error) = result {
      ui.message = Some(error.to_string());
    }
  }
  true
}

pub(super) fn scales(down: &[f64], up: &[f64], settings: &Settings) -> (f64, f64) {
  let mut down = if settings.net_auto() {
    auto_scale(down)
  } else {
    settings.net_download() as f64
  };
  let mut up = if settings.net_auto() {
    auto_scale(up)
  } else {
    settings.net_upload() as f64
  };
  if settings.net_sync() {
    down = down.max(up);
    up = down;
  }
  (down, up)
}

fn auto_scale(values: &[f64]) -> f64 {
  let mut maximum = 10240.0;
  let mut above = 0u8;
  let mut below = 0u8;
  let mut first = true;
  for (index, &value) in values.iter().enumerate() {
    if !value.is_finite() || value < 0.0 {
      continue;
    }
    if value > maximum {
      above += 1;
      below = below.saturating_sub(1);
    } else if maximum > 10240.0 && value < maximum / 10.0 {
      below += 1;
      above = above.saturating_sub(1);
    }
    if first || above >= 5 || below >= 5 {
      let recent = &values[index.saturating_sub(4)..=index];
      let (sum, count) = recent
        .iter()
        .filter(|value| value.is_finite() && **value >= 0.0)
        .fold((0.0, 0), |(sum, count), value| (sum + value, count + 1));
      maximum = (sum / count as f64 * if below >= 5 { 3.0 } else { 1.3 }).max(10240.0);
      above = 0;
      below = 0;
      first = false;
    }
  }
  maximum
}

#[cfg(test)]
mod tests {
  use serde_json::json;

  use super::*;

  #[test]
  fn it_prefers_connected_traffic_and_honors_explicit_interfaces() {
    let sample = |a: f64, z: f64, z_up: bool| {
      serde_json::from_value(json!({
        "cpu":{"cpu":{"usage":0}},
        "interfaces":{
          "a":{"link_state":"LINK_STATE_UP","received_bytes_rate":a},
          "z":{"link_state":if z_up { "LINK_STATE_UP" } else { "LINK_STATE_DOWN" },"received_bytes_rate":z}
        },
        "memory":{"physical_memory_total":1000,"physical_memory_available":500,"arc_size":100}
      }))
      .unwrap()
    };
    let settings = Settings::default();
    let state = crate::client::fixtures::with_samples(vec![sample(100.0, 1000.0, true), sample(100.0, 1000.0, true)]);
    assert_eq!(selected(&state, &settings), Some("z"));
    let explicit = settings.with_preference("net_iface", json!("a")).unwrap();
    assert_eq!(selected(&state, &explicit), Some("a"));
    let settings = explicit.with_preference("net_iface", json!("missing")).unwrap();
    assert_eq!(selected(&state, &settings), Some("z"));
    let down = crate::client::fixtures::with_samples(vec![sample(100.0, 1000.0, false); 2]);
    assert_eq!(selected(&down, &settings), Some("a"));
    let tied = crate::client::fixtures::with_samples(vec![sample(100.0, 100.0, true); 2]);
    assert_eq!(selected(&tied, &settings), Some("a"));
    assert_eq!(selected(&State::default(), &settings), None);
  }

  mod handle {
    use super::*;

    #[tokio::test]
    async fn it_persists_interfaces_and_keeps_reset_baselines_scoped_to_each_server() {
      let mut settings = Settings::default();
      let before = settings.clone();
      let directory = tempfile::tempdir().unwrap();
      let path = directory.path().join("config.toml");
      let state = crate::client::fixtures::state();
      let original = state.network_totals()["enp1s0"].downloaded();
      let mut ui = Ui::default();
      for _ in 0..2 {
        let (_, receiver) = tokio::sync::watch::channel(state.clone());
        ui.sessions.push(Some(super::super::super::Session {
          interval: tokio::sync::watch::channel(2000).0,
          receiver,
          task: tokio::spawn(std::future::pending()),
        }));
      }
      handle(&mut ui, &mut settings, &path, KeyCode::Char('n'));
      assert_eq!(settings.net_iface(), "enp1s0");
      crate::config::save_preferences(&path, &before, &settings).unwrap();
      assert_eq!(crate::config::load(&path).unwrap().net_iface(), "enp1s0");
      handle(&mut ui, &mut settings, &path, KeyCode::Char('z'));
      assert_eq!(ui.net_baselines[&(0, "enp1s0".into())].0, original);
      ui.selected = 1;
      handle(&mut ui, &mut settings, &path, KeyCode::Char('z'));
      assert_eq!(ui.net_baselines.len(), 2);
      handle(&mut ui, &mut settings, &path, KeyCode::Char('z'));
      assert_eq!(ui.net_baselines.len(), 1);
      assert_eq!(
        ui.sessions[0].as_ref().unwrap().receiver.borrow().network_totals()["enp1s0"].downloaded(),
        original
      );
      let metrics = serde_json::from_value(json!({
        "cpu":{"cpu":{"usage":0}},
        "interfaces":{"down":{"link_state":"LINK_STATE_DOWN"}, "up":{"link_state":"LINK_STATE_UP"}},
        "memory":{"physical_memory_total":1000,"physical_memory_available":400,"arc_size":500}
      }))
      .unwrap();
      let other = crate::client::fixtures::with_metrics(metrics);
      assert_eq!(selected(&other, &settings), Some("up"));
      assert_eq!(settings.net_iface(), "enp1s0");
      assert_eq!(selected(&State::default(), &settings), None);
      handle(&mut ui, &mut settings, &path, KeyCode::Char('a'));
      handle(&mut ui, &mut settings, &path, KeyCode::Char('y'));
      assert!(!settings.net_auto());
      assert!(settings.net_sync());
    }
  }

  mod scales {
    use super::*;

    #[test]
    fn it_uses_fixed_auto_and_shared_maxima_and_ignores_missing_samples() {
      let mut settings = Settings::default();
      assert_eq!(scales(&[5000.0, f64::NAN], &[2000.0], &settings), (10240.0, 10240.0));
      settings = settings.with_preference("net_sync", json!(true)).unwrap();
      assert_eq!(scales(&[5000.0], &[2000.0], &settings), (10240.0, 10240.0));
      settings = settings
        .with_preference("net_auto", json!(false))
        .unwrap()
        .with_preference("net_download", json!(1000))
        .unwrap()
        .with_preference("net_upload", json!(2000))
        .unwrap();
      assert_eq!(scales(&[5000.0], &[8000.0], &settings), (2000.0, 2000.0));
      settings = settings.with_preference("net_sync", json!(false)).unwrap();
      assert_eq!(scales(&[], &[], &settings), (1000.0, 2000.0));
      assert!(settings.with_preference("net_download", json!(0)).is_err());
    }

    #[test]
    fn it_recovers_from_bursts_without_rescaling_on_each_small_change() {
      let settings = Settings::default();
      let mut rates = vec![1000000.0];
      let initial = scales(&rates, &[], &settings).0;
      assert_eq!(initial, 1300000.0);
      rates.extend([1000.0; 4]);
      assert_eq!(scales(&rates, &[], &settings).0, initial);
      rates.push(1000.0);
      assert_eq!(scales(&rates, &[], &settings).0, 10240.0);
      rates.extend([20000.0; 4]);
      assert_eq!(scales(&rates, &[], &settings).0, 10240.0);
      rates.push(20000.0);
      assert_eq!(scales(&rates, &[], &settings).0, 26000.0);
      assert_eq!(scales(&[1000.0], &[], &settings).0, 10240.0);
      let synced = settings.with_preference("net_sync", json!(true)).unwrap();
      assert_eq!(scales(&rates, &[50000.0], &synced), (65000.0, 65000.0));
    }
  }
}
