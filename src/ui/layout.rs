use ratatui::{
  crossterm::event::KeyCode,
  layout::{Constraint, Layout, Rect},
};

use super::{Ui, apply_preferences};
use crate::config::Settings;

pub(super) fn handle(ui: &mut Ui, settings: &mut Settings, path: &std::path::Path, key: KeyCode) -> bool {
  let (key, visible) = match key {
    KeyCode::Char('1') => ("show_cpu", settings.show_cpu()),
    KeyCode::Char('2') => ("show_mem", settings.show_mem()),
    KeyCode::Char('3') => ("show_net", settings.show_net()),
    KeyCode::Char('4') => ("show_apps", settings.show_apps()),
    _ => return false,
  };
  let result = settings
    .with_preference(key, serde_json::Value::Bool(!visible))
    .map_err(super::Error::from)
    .and_then(|candidate| apply_preferences(ui, settings, path, candidate));
  ui.message = result.err().map(|error| error.to_string());
  true
}

pub(super) fn areas(area: Rect, settings: &Settings) -> [Rect; 4] {
  let mut panels = [Rect::default(); 4];
  let mut rest = area;
  if settings.show_cpu() {
    if settings.show_mem() || settings.show_net() || settings.show_apps() {
      let constraints = if settings.cpu_bottom() {
        [Constraint::Percentage(68), Constraint::Min(0)]
      } else {
        [Constraint::Percentage(32), Constraint::Min(0)]
      };
      let [top, bottom] = Layout::vertical(constraints).areas(rest);
      (panels[0], rest) = if settings.cpu_bottom() {
        (bottom, top)
      } else {
        (top, bottom)
      };
    } else {
      panels[0] = rest;
    }
  }
  if settings.show_apps() {
    if settings.show_mem() || settings.show_net() {
      let constraints = if settings.apps_left() {
        [Constraint::Percentage(55), Constraint::Min(0)]
      } else {
        [Constraint::Percentage(45), Constraint::Min(0)]
      };
      let [left, right] = Layout::horizontal(constraints).areas(rest);
      (panels[3], rest) = if settings.apps_left() {
        (left, right)
      } else {
        (right, left)
      };
    } else {
      panels[3] = rest;
    }
  }
  match (settings.show_mem(), settings.show_net()) {
    (true, true) => {
      let constraints = if settings.mem_below_net() {
        [Constraint::Percentage(40), Constraint::Min(0)]
      } else {
        [Constraint::Percentage(60), Constraint::Min(0)]
      };
      let [top, bottom] = Layout::vertical(constraints).areas(rest);
      (panels[1], panels[2]) = if settings.mem_below_net() {
        (bottom, top)
      } else {
        (top, bottom)
      };
    }
    (true, false) => panels[1] = rest,
    (false, true) => panels[2] = rest,
    _ => {}
  }
  panels
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn it_fills_the_available_area_without_overlaps_for_every_panel_combination() {
    for mask in 0..16 {
      for positions in 0..8 {
        let settings: Settings = serde_json::from_value(serde_json::json!({
          "show_cpu": mask & 1 != 0, "show_mem": mask & 2 != 0,
          "show_net": mask & 4 != 0, "show_apps": mask & 8 != 0,
          "cpu_bottom": positions & 1 != 0, "mem_below_net": positions & 2 != 0,
          "apps_left": positions & 4 != 0,
        }))
        .unwrap();
        for (width, height) in [(0, 0), (1, 1), (70, 21), (160, 45)] {
          let area = Rect::new(2, 3, width, height);
          let panels = areas(area, &settings);
          let mut cells = std::collections::HashSet::new();
          for (index, panel) in panels.iter().enumerate() {
            if mask & (1 << index) == 0 {
              assert!(panel.is_empty());
            }
            for y in panel.y..panel.bottom() {
              for x in panel.x..panel.right() {
                assert!(area.contains((x, y).into()));
                assert!(cells.insert((x, y)), "panels overlap");
              }
            }
          }
          assert_eq!(
            cells.len(),
            if mask == 0 { 0 } else { width as usize * height as usize }
          );
          if mask == 15 && width >= 70 {
            assert_eq!(panels[0].y > panels[3].y, settings.cpu_bottom());
            assert_eq!(panels[3].x < panels[1].x, settings.apps_left());
            assert_eq!(panels[1].y > panels[2].y, settings.mem_below_net());
          }
        }
      }
    }
  }
}
