use ratatui::{
  Frame,
  crossterm::event::KeyCode,
  layout::Rect,
  text::Line,
  widgets::{Paragraph, Wrap},
};
use serde_json::Value;

use super::{Ui, apply_preferences, input::Input, view::panel};
use crate::config::Settings;

#[derive(Default)]
pub(super) struct Options {
  category: usize,
  row: usize,
  editing: Option<Input>,
  error: Option<String>,
}

struct OptionRow {
  key: &'static str,
  label: &'static str,
  description: &'static str,
}

fn rows(category: usize) -> Vec<OptionRow> {
  match category {
    0 => vec![
      OptionRow {
        key: "save_on_exit",
        label: "Save on exit",
        description: "Save changed display preferences on exit. Server and trust changes always save immediately.",
      },
      OptionRow {
        key: "theme",
        label: "Theme path",
        description: "A btop theme file. Relative paths use the config directory. Empty uses Default.",
      },
      OptionRow {
        key: "update_ms",
        label: "Redraw interval (ms)",
        description: "Redraw every 100 to 86400000 ms. The API has a minimum collection interval of two seconds.",
      },
    ],
    _ => Vec::new(),
  }
}

fn parse(text: &str, current: &Value) -> Result<Value, String> {
  match current {
    Value::Bool(_) => text
      .parse::<bool>()
      .map(Value::Bool)
      .map_err(|_| "Use true or false.".into()),
    Value::Number(_) => text
      .parse::<u64>()
      .map(Value::from)
      .map_err(|_| "Enter a nonnegative whole number.".into()),
    Value::Null if text.is_empty() => Ok(Value::Null),
    Value::Null | Value::String(_) => Ok(Value::String(text.into())),
    _ => Err("This setting cannot be edited.".into()),
  }
}

fn text(value: &Value) -> String {
  match value {
    Value::Null => String::new(),
    Value::String(value) => value.clone(),
    _ => value.to_string(),
  }
}

pub(super) fn handle(ui: &mut Ui, settings: &mut Settings, path: &std::path::Path, key: KeyCode) -> bool {
  let rows = rows(ui.options.category);
  ui.options.row = ui.options.row.min(rows.len().saturating_sub(1));
  if let Some(mut input) = ui.options.editing.take() {
    match key {
      KeyCode::Esc => ui.options.error = None,
      KeyCode::Enter => {
        let result = (|| {
          let row = rows.get(ui.options.row).ok_or("No setting selected.")?;
          let current = settings.preferences().map_err(|error| error.to_string())?;
          let value = if row.key == "theme" && input.value.is_empty() {
            Value::Null
          } else {
            parse(&input.value, &current[row.key])?
          };
          let candidate = settings
            .with_preference(row.key, value)
            .map_err(|error| error.to_string())?;
          apply_preferences(ui, settings, path, candidate).map_err(|error| error.to_string())
        })();
        match result {
          Ok(()) => ui.options.error = None,
          Err(error) => {
            ui.options.error = Some(error);
            ui.options.editing = Some(input);
          }
        }
      }
      _ => {
        input.handle(key);
        ui.options.editing = Some(input);
      }
    }
    return true;
  }
  match key {
    KeyCode::Tab | KeyCode::Right => {
      ui.options.category = (ui.options.category + 1) % 5;
      ui.options.row = 0;
    }
    KeyCode::BackTab | KeyCode::Left => {
      ui.options.category = (ui.options.category + 4) % 5;
      ui.options.row = 0;
    }
    KeyCode::Down => ui.options.row = (ui.options.row + 1).min(rows.len().saturating_sub(1)),
    KeyCode::Up => ui.options.row = ui.options.row.saturating_sub(1),
    KeyCode::PageDown => ui.options.row = (ui.options.row + 5).min(rows.len().saturating_sub(1)),
    KeyCode::PageUp => ui.options.row = ui.options.row.saturating_sub(5),
    KeyCode::Home => ui.options.row = 0,
    KeyCode::End => ui.options.row = rows.len().saturating_sub(1),
    KeyCode::Enter | KeyCode::Char(' ') => {
      if let Some(row) = rows.get(ui.options.row) {
        match settings.preferences() {
          Ok(values) => {
            if let Value::Bool(value) = values[row.key] {
              let result = settings
                .with_preference(row.key, Value::Bool(!value))
                .map_err(super::Error::from)
                .and_then(|candidate| apply_preferences(ui, settings, path, candidate));
              ui.options.error = result.err().map(|error| error.to_string());
            } else {
              ui.options.editing = Some(Input::new(text(&values[row.key])));
              ui.options.error = None;
            }
          }
          Err(error) => ui.options.error = Some(error.to_string()),
        }
      }
    }
    _ => return false,
  }
  true
}

pub(super) fn draw(frame: &mut Frame, ui: &Ui, settings: &Settings) {
  let height = if frame.area().height >= 18 {
    frame.area().height.saturating_sub(10).min(32)
  } else {
    frame.area().height
  };
  let area = super::menu::overlay(frame, height, 78, &ui.theme);
  frame.render_widget(
    panel(" tab / shift-tab ", "cpu_box", &ui.theme).style(ui.theme.style()),
    area,
  );
  let inner = area.inner(ratatui::layout::Margin::new(1, 1));
  if inner.width == 0 || inner.height == 0 {
    return;
  }
  let categories = if inner.width >= 60 {
    ["General", "CPU", "Memory/ZFS", "Network", "Apps"]
  } else {
    ["Gen", "CPU", "Mem", "Net", "Apps"]
  };
  for (index, category) in categories.iter().enumerate() {
    let x = inner.x + inner.width * index as u16 / 5;
    let width = inner.width * (index as u16 + 1) / 5 - (x - inner.x);
    let label = if index == ui.options.category {
      format!("[{category}]")
    } else {
      category.to_string()
    };
    let style = ui.theme.style().fg(ui.theme.color(if index == ui.options.category {
      "hi_fg"
    } else {
      "main_fg"
    }));
    frame.render_widget(
      Paragraph::new(label).centered().style(style),
      Rect::new(x, inner.y, width, 1),
    );
  }
  let rows = rows(ui.options.category);
  let values = match settings.preferences() {
    Ok(values) => values,
    Err(error) => {
      frame.render_widget(Paragraph::new(error.to_string()), inner);
      return;
    }
  };
  let body = Rect::new(inner.x, inner.y + 1, inner.width, inner.height.saturating_sub(2));
  let wide = inner.width >= 48;
  let left = if wide {
    Rect::new(body.x, body.y, 28.min(body.width / 2), body.height)
  } else {
    Rect::new(body.x, body.y, body.width, body.height.saturating_sub(3))
  };
  let description_area = if wide {
    Rect::new(
      left.right() + 2,
      body.y,
      body.width.saturating_sub(left.width + 2),
      body.height,
    )
  } else {
    Rect::new(body.x, left.bottom(), body.width, body.height - left.height)
  };
  if wide {
    frame.render_widget(
      Paragraph::new("│\n".repeat(body.height as usize)).style(ui.theme.style().fg(ui.theme.color("div_line"))),
      Rect::new(left.right(), body.y, 1, body.height),
    );
  }
  let visible = left.height as usize / 2;
  let start = ui.options.row.saturating_sub(visible.saturating_sub(1));
  for (index, row) in rows.iter().enumerate().skip(start).take(visible) {
    let y = left.y + (index - start) as u16 * 2;
    let selected = index == ui.options.row;
    let style = if selected {
      ui.theme
        .style()
        .fg(ui.theme.color("selected_fg"))
        .bg(ui.theme.color("selected_bg"))
    } else {
      ui.theme.style()
    };
    let value = text(&values[row.key]);
    let value = if value.is_empty() { "Default".into() } else { value };
    let value = if selected { format!("‹ {value} ›") } else { value };
    let lines = vec![Line::from(row.label), Line::from(value)];
    frame.render_widget(
      Paragraph::new(lines).centered().style(style),
      Rect::new(left.x, y, left.width, 2),
    );
    if selected && let Some(input) = &ui.options.editing {
      let edit = Rect::new(left.x, y + 1, left.width, 1);
      frame.render_widget(Paragraph::new(" ".repeat(edit.width as usize)).style(style), edit);
      input.draw(frame, edit, true);
    }
  }
  if description_area.height > 0 {
    let description = ui.options.error.as_deref().unwrap_or_else(|| {
      rows
        .get(ui.options.row)
        .map_or("No display settings in this category yet.", |row| row.description)
    });
    frame.render_widget(
      Paragraph::new(description)
        .wrap(Wrap {
          trim: true,
        })
        .style(ui.theme.style()),
      description_area,
    );
  }
  if inner.height >= 2 {
    frame.render_widget(
      Paragraph::new("↑↓ select · Enter edit · Esc back")
        .centered()
        .style(ui.theme.style().fg(ui.theme.color("inactive_fg"))),
      Rect::new(inner.x, inner.bottom() - 1, inner.width, 1),
    );
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn it_keeps_values_below_names_and_descriptions_on_the_right() {
    use ratatui::{Terminal, backend::TestBackend};
    let ui = Ui::default();
    for (width, height) in [(1, 1), (20, 4), (48, 16), (80, 24), (160, 60)] {
      let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
      terminal.draw(|frame| draw(frame, &ui, &Settings::default())).unwrap();
      if width < 80 {
        continue;
      }
      let buffer = terminal.backend().buffer();
      let find = |needle: &str| {
        (0..height)
          .find_map(|y| {
            let line: String = (0..width).map(|x| buffer[(x, y)].symbol()).collect();
            line.find(needle).map(|byte| (line[..byte].chars().count(), y))
          })
          .unwrap()
      };
      let (name_x, name_y) = find("Save on exit");
      let (_, value_y) = find("‹ true ›");
      let (description_x, description_y) = find("Save changed display");
      assert_eq!(value_y, name_y + 1);
      assert_eq!(description_y, name_y);
      assert!(description_x > name_x + 12);
      assert!(find("████████╗").1 < name_y);
    }
  }

  #[test]
  fn it_validates_edits_and_cancels_without_changing_preferences() {
    let mut ui = Ui::default();
    let mut settings = Settings::default();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    handle(&mut ui, &mut settings, &path, KeyCode::End);
    handle(&mut ui, &mut settings, &path, KeyCode::Enter);
    ui.options.editing = Some(Input::new("0"));
    handle(&mut ui, &mut settings, &path, KeyCode::Enter);
    assert_eq!(settings.update_ms(), 2000);
    assert!(ui.options.error.is_some());
    assert!(ui.options.editing.is_some());
    handle(&mut ui, &mut settings, &path, KeyCode::Esc);
    assert!(ui.options.editing.is_none());
    handle(&mut ui, &mut settings, &path, KeyCode::Enter);
    ui.options.editing = Some(Input::new("500"));
    handle(&mut ui, &mut settings, &path, KeyCode::Enter);
    assert_eq!(settings.update_ms(), 500);
    handle(&mut ui, &mut settings, &path, KeyCode::Home);
    handle(&mut ui, &mut settings, &path, KeyCode::Enter);
    assert!(!settings.save_on_exit());
    handle(&mut ui, &mut settings, &path, KeyCode::Tab);
    assert_eq!(ui.options.row, 0);
    assert_eq!(ui.options.category, 1);
    assert!(!path.exists());
  }
}
