use ratatui::{
  crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
  layout::{Position, Rect},
};

use super::{Ui, menu::Menu};

pub(super) fn controls(
  frame: &mut ratatui::Frame,
  hits: &mut Vec<(Rect, Action)>,
  area: Rect,
  theme: &super::theme::Theme,
  labels: &[(&str, &str, &str, KeyCode)],
) {
  use ratatui::{
    style::Modifier,
    text::{Line, Span},
  };
  if area.is_empty() {
    return;
  }
  let mut x = area.x;
  for (before, hotkey, after, key) in labels {
    let line = Line::from(vec![
      Span::raw(format!(" {before}")),
      Span::styled(
        *hotkey,
        theme.style().fg(theme.color("hi_fg")).add_modifier(Modifier::BOLD),
      ),
      Span::raw(format!("{after} ")),
    ])
    .style(theme.style().fg(theme.color("title")));
    let width = line.width() as u16;
    if width > area.right().saturating_sub(x) {
      break;
    }
    let target = Rect::new(x, area.y, width, 1);
    frame.render_widget(line, target);
    hits.push((target, Action::Key(*key)));
    x += width;
  }
}

pub(super) fn hints(
  hits: &mut Vec<(Rect, Action)>,
  area: Rect,
  labels: &[(&str, KeyCode)],
) -> ratatui::text::Line<'static> {
  let mut text = String::new();
  let mut x = area.x;
  for (label, key) in labels {
    let label = format!(" {label} ");
    let width = (ratatui::text::Line::from(label.as_str()).width() as u16).min(area.right().saturating_sub(x));
    hits.push((Rect::new(x, area.y, width, 1), Action::Key(*key)));
    x += width;
    text.push_str(&label);
  }
  ratatui::text::Line::from(text)
}

#[derive(Clone, Copy)]
pub(super) enum Action {
  Key(KeyCode),
  Main(usize),
  Option(usize),
  Category(usize),
  Server(usize),
  App(usize),
  Scroll,
}

pub(super) fn key(ui: &mut Ui, event: MouseEvent, disabled: bool) -> Option<KeyEvent> {
  if disabled || ui.dialog.is_some() || ui.discovery.is_some() || ui.filtering {
    return None;
  }
  let action = ui
    .hits
    .iter()
    .rev()
    .find(|(area, _)| area.contains(Position::new(event.column, event.row)))?
    .1;
  let code = match event.kind {
    MouseEventKind::ScrollUp if !matches!(action, Action::Key(_) | Action::Category(_)) => KeyCode::Up,
    MouseEventKind::ScrollDown if !matches!(action, Action::Key(_) | Action::Category(_)) => KeyCode::Down,
    MouseEventKind::Down(MouseButton::Left) => match action {
      Action::Key(key) => key,
      Action::Main(row) => {
        ui.menu = Some(Menu::Main(row));
        KeyCode::Enter
      }
      Action::Option(row) => {
        if !ui.options.select_row(row) {
          return None;
        }
        KeyCode::Enter
      }
      Action::Category(category) => {
        ui.options.select_category(category);
        return None;
      }
      Action::Server(row) => {
        ui.selected = row;
        KeyCode::Enter
      }
      Action::App(row) => {
        ui.app_row = row;
        return None;
      }
      Action::Scroll => return None,
    },
    _ => return None,
  };
  Some(KeyEvent::new(code, KeyModifiers::NONE))
}

pub(super) fn rows(hits: &mut Vec<(Rect, Action)>, area: Rect, offset: usize, count: usize, server: bool) {
  hits.push((area, Action::Scroll));
  for row in 0..(area.height as usize).min(count.saturating_sub(offset)) {
    hits.push((
      Rect::new(area.x, area.y + row as u16, area.width, 1),
      if server {
        Action::Server(offset + row)
      } else {
        Action::App(offset + row)
      },
    ));
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn it_uses_visible_hits_and_respects_disabled_input_and_dialogs() {
    let mut ui = Ui::default();
    let event = MouseEvent {
      kind: MouseEventKind::Down(MouseButton::Left),
      column: 2,
      row: 2,
      modifiers: KeyModifiers::NONE,
    };
    ui.hits.push((Rect::new(1, 1, 5, 5), Action::Main(1)));
    assert_eq!(key(&mut ui, event, false).unwrap().code, KeyCode::Enter);
    assert_eq!(ui.menu, Some(Menu::Main(1)));
    let scroll = MouseEvent {
      kind: MouseEventKind::ScrollDown,
      ..event
    };
    assert_eq!(key(&mut ui, scroll, false).unwrap().code, KeyCode::Down);
    assert!(key(&mut ui, event, true).is_none());
    ui.dialog = Some(super::super::manager::Dialog::Delete);
    assert!(key(&mut ui, event, false).is_none());
    ui.dialog = None;
    ui.hits.clear();
    assert!(key(&mut ui, event, false).is_none());
  }

  #[test]
  fn it_hits_large_menu_choices_and_both_setting_lines_after_resizing() {
    use ratatui::{Terminal, backend::TestBackend};
    let mut ui = Ui::default();
    ui.picker = true;
    let settings = crate::config::Settings::default();
    for (width, height) in [(80, 24), (160, 60), (20, 4)] {
      let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
      ui.menu = Some(Menu::Main(4));
      terminal
        .draw(|frame| super::super::view::draw(frame, &mut ui, &settings))
        .unwrap();
      let area = ui
        .hits
        .iter()
        .find_map(|(area, action)| matches!(action, Action::Main(4)).then_some(*area))
        .unwrap();
      let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: area.x,
        row: area.y + area.height - 1,
        modifiers: KeyModifiers::NONE,
      };
      let event = key(&mut ui, click, false).unwrap();
      assert_eq!(super::super::menu::handle(&mut ui, event.code), Some(true));
      if width < 80 {
        continue;
      }
      ui.menu = Some(Menu::Options);
      terminal
        .draw(|frame| super::super::view::draw(frame, &mut ui, &settings))
        .unwrap();
      let area = ui
        .hits
        .iter()
        .find_map(|(area, action)| matches!(action, Action::Option(0)).then_some(*area))
        .unwrap();
      assert_eq!(area.height, 2);
      for row in area.y..area.bottom() {
        let click = MouseEvent {
          column: area.x,
          row,
          ..click
        };
        assert_eq!(key(&mut ui, click, false).unwrap().code, KeyCode::Enter);
      }
      let description = MouseEvent {
        column: area.right() + 3,
        row: area.y,
        ..click
      };
      assert!(key(&mut ui, description, false).is_none());
    }
  }

  #[test]
  fn it_discards_dashboard_targets_when_an_overlay_opens_or_resizes() {
    use ratatui::{Terminal, backend::TestBackend};
    let mut ui = Ui::default();
    ui.picker = true;
    ui.menu = Some(Menu::Main(0));
    for (width, height) in [(80, 24), (20, 4)] {
      let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
      terminal
        .draw(|frame| super::super::view::draw(frame, &mut ui, &crate::config::Settings::default()))
        .unwrap();
      assert!(!ui.hits.is_empty());
      assert!(
        ui.hits
          .iter()
          .all(|(_, action)| matches!(action, Action::Main(_) | Action::Scroll))
      );
    }
  }
}
