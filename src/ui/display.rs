use std::collections::HashMap;

use ratatui::{Frame, style::Color};

pub(super) fn indexed(color: Color, tty: bool) -> Color {
  let Color::Rgb(r, g, b) = color else { return color };
  let range = if tty { 0..16 } else { 16..256 };
  let index = range
    .min_by_key(|index| {
      let (red, green, blue) = match *index {
        0..16 => [
          (0, 0, 0),
          (128, 0, 0),
          (0, 128, 0),
          (128, 128, 0),
          (0, 0, 128),
          (128, 0, 128),
          (0, 128, 128),
          (192, 192, 192),
          (128, 128, 128),
          (255, 0, 0),
          (0, 255, 0),
          (255, 255, 0),
          (0, 0, 255),
          (255, 0, 255),
          (0, 255, 255),
          (255, 255, 255),
        ][*index as usize],
        16..232 => {
          let cube = index - 16;
          let levels = [0, 95, 135, 175, 215, 255];
          (
            levels[(cube / 36) as usize],
            levels[(cube / 6 % 6) as usize],
            levels[(cube % 6) as usize],
          )
        }
        _ => {
          let gray = 8 + (index - 232) * 10;
          (gray, gray, gray)
        }
      };
      (i32::from(r) - red).pow(2) + (i32::from(g) - green).pow(2) + (i32::from(b) - blue).pow(2)
    })
    .unwrap();
  Color::Indexed(index as u8)
}

pub(super) fn apply(frame: &mut Frame, settings: &crate::config::Settings, tty: bool) {
  if settings.truecolor() && !tty && settings.rounded_corners() {
    return;
  }
  let mut colors = HashMap::new();
  for cell in &mut frame.buffer_mut().content {
    if !settings.truecolor() || tty {
      cell.fg = *colors.entry(cell.fg).or_insert_with(|| indexed(cell.fg, tty));
      cell.bg = *colors.entry(cell.bg).or_insert_with(|| indexed(cell.bg, tty));
    }
    if !settings.rounded_corners() {
      let corner = match cell.symbol() {
        "╭" => Some("┌"),
        "╮" => Some("┐"),
        "╰" => Some("└"),
        "╯" => Some("┘"),
        _ => None,
      };
      if let Some(corner) = corner {
        cell.set_symbol(corner);
      }
    }
    if tty && !cell.symbol().is_ascii() {
      let text: String = cell
        .symbol()
        .chars()
        .map(|character| match character {
          '─' | '═' | '╴' | '╶' => '-',
          '│' | '║' | '╵' | '╷' => '|',
          '↑' | '▲' => '^',
          '↓' | '▼' => 'v',
          '←' | '‹' => '<',
          '→' | '›' => '>',
          '×' => 'x',
          '·' | '…' => '.',
          '°' => '*',
          '\u{2500}'..='\u{257f}' => '+',
          '\u{2580}'..='\u{259f}' | '\u{2801}'..='\u{28ff}' => '#',
          '\u{2800}' => ' ',
          character if character.is_ascii() => character,
          _ => '?',
        })
        .collect();
      cell.set_symbol(&text);
    }
  }
}

#[cfg(test)]
mod tests {
  mod apply {
    use pretty_assertions::assert_eq;

    use super::super::*;

    #[test]
    fn it_changes_corners_without_losing_rgb_and_keeps_tty_ascii() {
      let settings = crate::config::Settings::default()
        .with_preference("rounded_corners", serde_json::json!(false))
        .unwrap();
      for tty in [false, true] {
        let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(4, 1)).unwrap();
        terminal
          .draw(|frame| {
            frame.render_widget(
              ratatui::widgets::Paragraph::new("╭╮╰╯")
                .style(ratatui::style::Style::default().fg(Color::Rgb(12, 34, 56))),
              frame.area(),
            );
            apply(frame, &settings, tty);
          })
          .unwrap();
        let cells = &terminal.backend().buffer().content;
        let symbols: String = cells.iter().map(|cell| cell.symbol()).collect();
        assert_eq!(symbols, if tty { "++++" } else { "┌┐└┘" });
        if !tty {
          assert_eq!(cells[0].fg, Color::Rgb(12, 34, 56));
        }
      }
    }
  }

  use super::*;

  #[test]
  fn it_maps_rgb_to_the_nearest_cube_or_gray_and_keeps_terminal_defaults() {
    assert_eq!(indexed(Color::Rgb(255, 0, 0), false), Color::Indexed(196));
    assert_eq!(indexed(Color::Rgb(128, 128, 128), false), Color::Indexed(244));
    assert_eq!(indexed(Color::Rgb(255, 0, 0), true), Color::Indexed(9));
    assert_eq!(indexed(Color::Reset, false), Color::Reset);
  }
}
