use ratatui::{
  Frame,
  crossterm::event::KeyCode,
  layout::Rect,
  style::Style,
  text::{Line, Text},
  widgets::{Clear, Paragraph},
};

use super::{Ui, view::panel};

#[derive(Debug, PartialEq)]
pub(super) enum Menu {
  Main(usize),
  Options,
  Help,
  Update,
}

pub(super) fn handle(ui: &mut Ui, key: KeyCode) -> Option<bool> {
  if ui.dialog.is_some() || ui.discovery.is_some() || ui.filtering {
    return None;
  }
  match key {
    KeyCode::Esc | KeyCode::Char('m') => {
      ui.menu = match ui.menu {
        Some(Menu::Main(_)) => None,
        _ => Some(Menu::Main(0)),
      };
    }
    KeyCode::F(2) | KeyCode::Char('o') => ui.menu = Some(Menu::Options),
    KeyCode::F(1) | KeyCode::Char('?') | KeyCode::Char('h' | 'H') => ui.menu = Some(Menu::Help),
    KeyCode::Char('s') => {
      ui.menu = None;
      ui.picker = true;
    }
    KeyCode::Down if matches!(ui.menu, Some(Menu::Main(_))) => {
      if let Some(Menu::Main(row)) = &mut ui.menu {
        *row = (*row + 1) % 5;
      }
    }
    KeyCode::Up if matches!(ui.menu, Some(Menu::Main(_))) => {
      if let Some(Menu::Main(row)) = &mut ui.menu {
        *row = (*row + 4) % 5;
      }
    }
    KeyCode::Enter if matches!(ui.menu, Some(Menu::Main(_))) => match ui.menu {
      Some(Menu::Main(0)) => ui.menu = Some(Menu::Options),
      Some(Menu::Main(1)) => ui.menu = Some(Menu::Help),
      Some(Menu::Main(2)) => {
        ui.menu = None;
        ui.picker = true;
      }
      Some(Menu::Main(3)) => {
        ui.update.start();
        ui.menu = Some(Menu::Update);
      }
      Some(Menu::Main(4)) => return Some(true),
      _ => {}
    },
    KeyCode::Char('q') if matches!(ui.menu, Some(Menu::Main(_))) => return Some(true),
    _ if ui.menu.is_some() => {}
    _ => return None,
  }
  Some(false)
}

pub(super) fn draw(frame: &mut Frame, ui: &Ui) {
  let Some(menu) = &ui.menu else { return };
  if let Menu::Main(selected) = menu {
    let rows = ["Options", "Help", "Servers", "Update", "Quit"];
    let large = frame.area().width >= 44 && frame.area().height >= 24;
    let stride = if large { if frame.area().height >= 28 { 4 } else { 3 } } else { 1 };
    let area = overlay(frame, rows.len() as u16 * stride, 44, &ui.theme);
    let visible = (area.height / stride).max(1) as usize;
    let start = selected.saturating_sub(visible.saturating_sub(1));
    for (index, label) in rows.iter().enumerate().skip(start).take(visible) {
      let row = Rect::new(
        area.x,
        area.y + (index - start) as u16 * stride,
        area.width,
        stride.min(area.height),
      );
      let style = ui
        .theme
        .style()
        .fg(ui.theme.color(if index == *selected { "hi_fg" } else { "main_fg" }));
      let text = if large { lettering(label) } else { Text::from(*label) };
      frame.render_widget(Paragraph::new(text).centered().style(style), row);
    }
    return;
  }
  if *menu == Menu::Update {
    let area = overlay(frame, 9, 74, &ui.theme);
    frame.render_widget(
      Paragraph::new(format!("{}\n\nEsc returns to Main.", ui.update.message()))
        .wrap(ratatui::widgets::Wrap { trim: true })
        .style(ui.theme.style())
        .block(panel(" update ", "cpu_box", &ui.theme)),
      area,
    );
    return;
  }
  let (title, rows, selected) = match menu {
    Menu::Main(row) => (
      " main ",
      vec!["Options", "Help", "Servers", "Update", "Quit"],
      Some(*row),
    ),
    Menu::Options => (
      " options ",
      vec!["Options are not available yet.", "Esc returns to Main."],
      None,
    ),
    Menu::Help => (" help ", vec!["Esc returns to Main.", "Ctrl-C quits."], None),
    Menu::Update => return,
  };
  let bounds = frame.area();
  let width = bounds.width.min(44);
  let height = bounds.height.min(rows.len() as u16 + 2);
  let area = Rect::new(
    bounds.x + (bounds.width - width) / 2,
    bounds.y + (bounds.height - height) / 2,
    width,
    height,
  );
  frame.render_widget(Clear, area);
  let lines: Vec<_> = rows
    .into_iter()
    .enumerate()
    .map(|(index, text)| {
      let style = if selected == Some(index) {
        Style::default()
          .fg(ui.theme.color("selected_fg"))
          .bg(ui.theme.color("selected_bg"))
      } else {
        ui.theme.style()
      };
      Line::styled(
        format!(" {} {text}", if selected == Some(index) { "›" } else { " " }),
        style,
      )
    })
    .collect();
  frame.render_widget(
    Paragraph::new(lines)
      .style(ui.theme.style())
      .block(panel(title, "cpu_box", &ui.theme)),
    area,
  );
}

pub(super) fn overlay(frame: &mut Frame, height: u16, width: u16, theme: &super::Theme) -> Rect {
  let bounds = frame.area();
  let banner = Text::from(&super::Banner::new().with_version());
  let banner_height = if bounds.width >= banner.width() as u16 && bounds.height >= height + 8 {
    8
  } else {
    0
  };
  let height = height.min(bounds.height.saturating_sub(banner_height));
  let width = width.min(bounds.width);
  let y = bounds.y + (bounds.height - height - banner_height) / 2;
  if banner_height > 0 {
    let area = Rect::new(
      bounds.x + (bounds.width - banner.width() as u16) / 2,
      y,
      banner.width() as u16,
      7,
    );
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(banner).style(theme.style()), area);
  }
  let area = Rect::new(bounds.x + (bounds.width - width) / 2, y + banner_height, width, height);
  frame.render_widget(Clear, area);
  frame.render_widget(Paragraph::new("").style(theme.style()), area);
  area
}

fn lettering(label: &str) -> Text<'static> {
  let mut lines = [String::new(), String::new(), String::new()];
  for letter in label.chars() {
    let glyph = match letter.to_ascii_uppercase() {
      'A' => ["┌─┐", "├─┤", "╵ ╵"],
      'D' => ["┌─┐", "│ │", "└─┘"],
      'E' => ["┌─╴", "├─╴", "└─╴"],
      'H' => ["╷ ╷", "├─┤", "╵ ╵"],
      'I' => ["─┬─", " │ ", "─┴─"],
      'L' => ["╷  ", "│  ", "└─╴"],
      'N' => ["┌┐╷", "│││", "╵└┘"],
      'O' => ["┌─┐", "│ │", "└─┘"],
      'P' => ["┌─┐", "├─┘", "╵  "],
      'Q' => ["┌─┐", "│ │", "└─┼"],
      'R' => ["┌─┐", "├┬┘", "╵└╴"],
      'S' => ["┌─╴", "└─┐", "╶─┘"],
      'T' => ["─┬─", " │ ", " ╵ "],
      'U' => ["╷ ╷", "│ │", "└─┘"],
      'V' => ["╷ ╷", "│ │", "└┬┘"],
      _ => ["   "; 3],
    };
    for (line, glyph) in lines.iter_mut().zip(glyph) {
      if !line.is_empty() {
        line.push(' ');
      }
      line.push_str(glyph);
    }
  }
  Text::from(lines.into_iter().map(Line::from).collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn it_returns_one_menu_layer_and_keeps_dashboard_selection() {
    let mut ui = Ui::default();
    ui.selected = 2;
    ui.app_row = 5;
    assert_eq!(handle(&mut ui, KeyCode::Char('m')), Some(false));
    assert_eq!(ui.menu, Some(Menu::Main(0)));
    handle(&mut ui, KeyCode::Enter);
    assert_eq!(ui.menu, Some(Menu::Options));
    handle(&mut ui, KeyCode::Esc);
    assert_eq!(ui.menu, Some(Menu::Main(0)));
    handle(&mut ui, KeyCode::Esc);
    assert_eq!(ui.menu, None);
    assert_eq!((ui.selected, ui.app_row), (2, 5));
    handle(&mut ui, KeyCode::F(1));
    assert_eq!(ui.menu, Some(Menu::Help));
    handle(&mut ui, KeyCode::Char('s'));
    assert!(ui.picker);
    assert_eq!(ui.menu, None);
  }

  #[test]
  fn it_leaves_editing_keys_to_the_active_field() {
    let mut ui = Ui::default();
    ui.dialog = Some(super::super::manager::Dialog::Add {
      field: 0,
      values: Default::default(),
    });
    for key in [KeyCode::Esc, KeyCode::Char('o'), KeyCode::Char('s')] {
      assert_eq!(handle(&mut ui, key), None);
    }
    assert_eq!(ui.menu, None);
    ui.dialog = None;
    ui.filtering = true;
    assert_eq!(handle(&mut ui, KeyCode::Char('m')), None);
  }

  #[test]
  fn it_draws_menus_in_tiny_terminals() {
    use ratatui::{Terminal, backend::TestBackend};
    let mut ui = Ui::default();
    ui.menu = Some(Menu::Main(0));
    for (width, height) in [(1, 1), (20, 4), (80, 24)] {
      let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
      terminal.draw(|frame| draw(frame, &ui)).unwrap();
    }
  }

  #[test]
  fn it_centers_the_banner_above_large_menu_choices() {
    use ratatui::{Terminal, backend::TestBackend};
    let mut ui = Ui::default();
    ui.menu = Some(Menu::Main(0));
    for (width, height) in [(80, 32), (160, 60)] {
      let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
      terminal.draw(|frame| draw(frame, &ui)).unwrap();
      let buffer = terminal.backend().buffer();
      let lines: Vec<String> = (0..height)
        .map(|y| (0..width).map(|x| buffer[(x, y)].symbol()).collect())
        .collect();
      let banner = lines.iter().position(|line| line.contains("████████╗")).unwrap();
      let options = lines.iter().position(|line| line.contains("┌─┐ ┌─┐ ─┬─")).unwrap();
      assert!(options >= banner + 8);
      assert_eq!(buffer[((width - 27) / 2, options as u16)].fg, ui.theme.color("hi_fg"));
    }
  }
}
