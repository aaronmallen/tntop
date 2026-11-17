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
  page_size: usize,
  editing: Option<Input>,
  theme_choice: Option<ThemeChoice>,
  error: Option<String>,
}

struct ThemeChoice {
  names: Vec<String>,
  index: usize,
  original: super::Theme,
  candidate: Option<Settings>,
}

struct OptionRow {
  key: &'static str,
  label: &'static str,
  description: &'static str,
}

impl Options {
  pub fn editing_text(&self) -> bool {
    self.editing.is_some()
  }

  pub fn select_row(&mut self, row: usize) -> bool {
    if self.editing.is_some() || self.theme_choice.is_some() {
      return false;
    }
    self.row = row.min(rows(self.category).len().saturating_sub(1));
    true
  }

  pub fn select_category(&mut self, category: usize) {
    if self.editing.is_none() && self.theme_choice.is_none() {
      self.category = category.min(4);
      self.row = 0;
    }
  }
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
        label: "Color theme",
        description: "Enter to browse built-in, bundled and user themes. Arrows preview, Enter keeps the choice, Esc restores the previous theme. Explicit paths remain available in the config.",
      },
      OptionRow {
        key: "theme_background",
        label: "Theme background",
        description: "Use the theme's background color. Disable to use your terminal background.",
      },
      OptionRow {
        key: "truecolor",
        label: "Truecolor",
        description: "Use RGB colors. Disable for a 256-color terminal. Applies to panels, gradients, banners and dialogs.",
      },
      OptionRow {
        key: "force_tty",
        label: "Force TTY",
        description: "Use a 16-color palette, ASCII labels and TTY graphs. Turning this off restores the selected theme and graph choices.",
      },
      OptionRow {
        key: "disable_mouse",
        label: "Disable mouse",
        description: "Use keyboard controls only. Release mouse capture so the terminal can select text.",
      },
      OptionRow {
        key: "update_ms",
        label: "Redraw interval (ms)",
        description: "Redraw every 100 to 86400000 ms. The API has a minimum collection interval of two seconds.",
      },
      OptionRow {
        key: "graph_symbol",
        label: "Graph symbol",
        description: "Enter cycles braille, block and tty. Braille and block show two samples per column; tty shows one. Panel overrides can inherit this choice.",
      },
      OptionRow {
        key: "presets",
        label: "Presets",
        description: "Up to nine layouts, separated by spaces. Each layout lists panel:position:graph entries separated by commas. Panels: cpu, mem, net, apps (or proc). Position: 0/1. Graph: default, braille, block, tty.",
      },
      OptionRow {
        key: "disable_presets",
        label: "Disable presets",
        description: "Enter cycles off, default, custom and all. Default disables preset 0; custom disables presets 1–9. Cycling skips disabled choices.",
      },
      OptionRow {
        key: "rounded_corners",
        label: "Rounded corners",
        description: "Draw rounded panel corners. TTY output always uses ASCII borders.",
      },
      OptionRow {
        key: "tty_sync",
        label: "Synchronized output",
        description: "Keep the previous frame visible until the next frame is complete. Disable if your terminal does not support synchronized updates. Off in TTY mode.",
      },
      OptionRow {
        key: "vim_keys",
        label: "Vim keys",
        description: "Use h/j/k/l as Left/Down/Up/Right in lists and menus. H still opens Help. Text fields keep these letters as text.",
      },
      OptionRow {
        key: "base_10_sizes",
        label: "Base-10 sizes",
        description: "Use decimal byte units (kB, MB, GB) instead of binary units (KiB, MiB, GiB). Applies to memory, storage, network and apps.",
      },
      OptionRow {
        key: "clock_format",
        label: "Clock format",
        description: "UTC strftime format, or empty to hide. /host is the configured server URL host. /user is N/A (API identity unavailable). /uptime uses TrueNAS system uptime.",
      },
      OptionRow {
        key: "background_update",
        label: "Background update",
        description: "Update the dashboard behind menus. When off, keep its last frame while collection continues. Resizing redraws it once.",
      },
      OptionRow {
        key: "log_level",
        label: "Log level",
        description: "Cycle OFF, ERROR, WARN, INFO, DEBUG and TRACE. During the TUI, diagnostics append to tntop.log beside the config file.",
      },
    ],
    1..=4 => {
      let mut rows = vec![
        OptionRow {
          key: [
            "graph_symbol_cpu",
            "graph_symbol_mem",
            "graph_symbol_net",
            "graph_symbol_apps",
          ][category - 1],
          label: "Graph symbol",
          description: "Enter cycles default, braille, block and tty. Default inherits the General setting. Applies to charts and sparklines without clearing history.",
        },
        OptionRow {
          key: ["show_cpu", "show_mem", "show_net", "show_apps"][category - 1],
          label: "Show panel",
          description: "Show this panel on the dashboard. Keys 1–4 toggle panels. Hidden panels keep collecting data.",
        },
      ];
      let position = match category {
        1 => Some(OptionRow {
          key: "cpu_bottom",
          label: "CPU at bottom",
          description: "Place the CPU panel below the other panels.",
        }),
        2 => Some(OptionRow {
          key: "mem_below_net",
          label: "Memory below network",
          description: "Place Memory/ZFS below Network. Storage stays inside Memory/ZFS.",
        }),
        4 => Some(OptionRow {
          key: "apps_left",
          label: "Apps on left",
          description: "Place Apps to the left of Memory/ZFS and Network.",
        }),
        _ => None,
      };
      rows.extend(position);
      if category == 1 {
        rows.extend([
          OptionRow { key: "cpu_graph_upper", label: "Upper graph source", description: "Cycle Auto, total, busiest core and available cores. Auto uses total. A saved core absent on this server shows N/A." },
          OptionRow { key: "cpu_graph_lower", label: "Lower graph source", description: "Cycle Auto, total, busiest core and available cores. Auto uses busiest core. Missing cores stay unavailable rather than falling back." },
          OptionRow { key: "cpu_invert_lower", label: "Invert lower graph", description: "Draw the lower graph down from its top edge." },
          OptionRow { key: "cpu_single_graph", label: "Single graph", description: "Use the whole graph area for the upper source and hide the lower graph." },
          OptionRow { key: "check_temp", label: "Show temperatures", description: "Show existing CPU temperature readings. This does not enable new sensors or collection." },
          OptionRow { key: "show_coretemp", label: "Show core temperatures", description: "Show per-core temperatures when Show temperatures is on. Missing readings show N/A." },
          OptionRow { key: "temp_scale", label: "Temperature units", description: "Cycle Celsius, Fahrenheit, Kelvin and Rankine. The existing readings arrive in Celsius." },
          OptionRow { key: "custom_cpu_name", label: "Custom CPU name", description: "Replace the model label. Empty uses the model reported by TrueNAS." },
          OptionRow { key: "show_uptime", label: "Show uptime", description: "Show the existing TrueNAS uptime reading below the CPU graphs." },
        ]);
      }
      if category == 2 {
        rows.extend([
          OptionRow { key: "mem_graphs", label: "Memory graphs", description: "Show history graphs for memory values. Turn off for meters with the same current values and total-RAM scale." },
          OptionRow { key: "show_disks", label: "Show storage", description: "Show pools and aggregate disk I/O beside memory. Turn off to give memory the whole panel." },
          OptionRow { key: "show_arc", label: "Show ARC/cache", description: "Show ARC cache size and reported ARC-free memory separately. These overlap RAM; never add them to Used or Available. Turn off for RAM-only presentation." },
          OptionRow { key: "disks_filter", label: "Pool filter", description: "Space-separated pool-name fragments to include. Prefix exclude= to exclude matches. Empty shows all pools. Brackets select within matching pools." },
          OptionRow { key: "show_io_stat", label: "Show disk I/O", description: "Show existing aggregate disk statistics. Turn off to give pool capacity the full storage area." },
          OptionRow { key: "io_mode", label: "I/O view", description: "Use the storage area for aggregate read/write graphs instead of pool capacity and busy history. These are all-disk values, not selected-pool measurements." },
          OptionRow { key: "io_graph_combined", label: "Combined I/O graph", description: "In I/O view, combine read and write rates. Turn off for read above and write below on one shared scale." },
          OptionRow { key: "io_graph_scale", label: "I/O scale (bytes/s)", description: "Fixed graph maximum in bytes per second. Zero scales to retained history. Combined mode scales read plus write, separate mode scales each direction." },
        ]);
      }
      if category == 3 {
        rows.extend([
          OptionRow { key: "net_iface", label: "Preferred interface", description: "Interface name to prefer on each server. Empty or absent falls back to an up interface, then the first available. b/n cycles and saves this preference." },
          OptionRow { key: "net_auto", label: "Automatic scale", description: "Scale each graph to retained history. Turn off for fixed byte-per-second maxima. The a key toggles this." },
          OptionRow { key: "net_sync", label: "Sync scales", description: "Use the larger download/upload maximum for both graphs. The y key toggles this in either automatic or fixed mode." },
          OptionRow { key: "net_download", label: "Download scale (bytes/s)", description: "Positive fixed maximum in bytes per second. Used when automatic scaling is off." },
          OptionRow { key: "net_upload", label: "Upload scale (bytes/s)", description: "Positive fixed maximum in bytes per second. Used when automatic scaling is off." },
          OptionRow { key: "net_swap", label: "Swap graph positions", description: "Show upload above download. Arrows follow graph orientation; labels, gradients, rates and counters stay with their traffic." },
          OptionRow { key: "net_bitrate", label: "Bitrate units", description: "Show rates in bits per second, using the General base-10/binary preference. Session estimates stay in bytes. z toggles a local reset baseline for this server/interface." },
        ]);
      }
      if category == 4 {
        rows.extend([
          OptionRow { key: "apps_sort", label: "Sort apps by", description: "Cycle CPU, memory, name and state. Left/Right also cycles. Missing stats stay last; ties sort by name." },
          OptionRow { key: "apps_reversed", label: "Reverse app order", description: "Reverse the selected sort field. The r key toggles this. Selection follows app identity." },
          OptionRow { key: "apps_colors", label: "App colors", description: "Use state, update and resource colors. Turn off for plain rows; selection stays highlighted." },
          OptionRow { key: "apps_gradient", label: "Resource gradients", description: "Color CPU and memory values by their share of host resources when App colors is on." },
          OptionRow { key: "apps_mem_bytes", label: "Memory in bytes", description: "Show byte units. Turn off for percent of remote physical RAM. Missing app stats or RAM remain N/A." },
          OptionRow { key: "apps_per_core", label: "Per-core CPU", description: "Multiply reported host CPU percent by the current logical core count. CORE% can exceed 100. Missing core count shows N/A. Graphs always show host capacity." },
          OptionRow { key: "apps_cpu_graphs", label: "App CPU graphs", description: "Show per-app CPU history when space allows. Graph style follows the Apps graph-symbol setting." },
        ]);
      }
      rows
    }
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
  if let Some(mut choice) = ui.options.theme_choice.take() {
    match key {
      KeyCode::Esc => {
        ui.theme = choice.original;
        ui.options.error = None;
        return true;
      }
      KeyCode::Enter => {
        if let Some(candidate) = choice.candidate.take() {
          match apply_preferences(ui, settings, path, candidate) {
            Ok(()) => {
              ui.options.error = None;
              return true;
            }
            Err(error) => ui.options.error = Some(error.to_string()),
          }
        }
      }
      KeyCode::Left | KeyCode::Up | KeyCode::Right | KeyCode::Down | KeyCode::Home | KeyCode::End => {
        choice.index = match key {
          KeyCode::Left | KeyCode::Up => (choice.index + choice.names.len() - 1) % choice.names.len(),
          KeyCode::Home => 0,
          KeyCode::End => choice.names.len() - 1,
          _ => (choice.index + 1) % choice.names.len(),
        };
        choice.candidate = None;
        let result = settings
          .with_preference("theme", Value::String(choice.names[choice.index].clone()))
          .map_err(super::Error::from)
          .and_then(|candidate| super::load_theme(&candidate, path).map(|theme| (candidate, theme)));
        match result {
          Ok((candidate, theme)) => {
            ui.theme = theme;
            choice.candidate = Some(candidate);
            ui.options.error = None;
          }
          Err(error) => ui.options.error = Some(error.to_string()),
        }
      }
      _ => {}
    }
    ui.options.theme_choice = Some(choice);
    return true;
  }
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
    KeyCode::Tab => {
      ui.options.category = (ui.options.category + 1) % 5;
      ui.options.row = 0;
    }
    KeyCode::BackTab => {
      ui.options.category = (ui.options.category + 4) % 5;
      ui.options.row = 0;
    }
    KeyCode::Char(category @ '1'..='5') => ui.options.select_category(category as usize - '1' as usize),
    KeyCode::Down => ui.options.row = (ui.options.row + 1) % rows.len(),
    KeyCode::Up => ui.options.row = (ui.options.row + rows.len() - 1) % rows.len(),
    KeyCode::PageDown | KeyCode::PageUp => {
      let size = ui.options.page_size.max(1);
      let pages = rows.len().div_ceil(size);
      let page = ui.options.row / size;
      ui.options.row = ((page + if key == KeyCode::PageDown { 1 } else { pages - 1 }) % pages) * size;
    }
    KeyCode::Home => ui.options.row = 0,
    KeyCode::End => ui.options.row = rows.len().saturating_sub(1),
    KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right => {
      let previous = key == KeyCode::Left;
      if let Some(row) = rows.get(ui.options.row) {
        match settings.preferences() {
          Ok(values) => {
            if row.key == "theme" {
              match super::Theme::names(path) {
                Ok(mut names) => {
                  let current = settings
                    .theme()
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "Default".into());
                  let index = names.iter().position(|name| *name == current).unwrap_or_else(|| {
                    names.push(current);
                    names.len() - 1
                  });
                  ui.options.theme_choice = Some(ThemeChoice {
                    names,
                    index,
                    original: ui.theme.clone(),
                    candidate: Some(settings.clone()),
                  });
                  ui.options.error = None;
                }
                Err(error) => ui.options.error = Some(error.to_string()),
              }
            } else if matches!(row.key, "cpu_graph_upper" | "cpu_graph_lower") {
              let mut choices = vec!["auto".to_owned(), "total".into(), "busiest".into()];
              if let Some(Some(session)) = ui.sessions.get(ui.selected)
                && let Some(metrics) = session.receiver.borrow().history().back()
              {
                choices.extend(metrics.cpu().keys().filter(|name| name.as_str() != "cpu").cloned());
              }
              let index = choices.iter().position(|choice| values[row.key] == choice.as_str());
              let next = index.map_or(0, |index| {
                (index + if previous { choices.len() - 1 } else { 1 }) % choices.len()
              });
              let result = settings
                .with_preference(row.key, Value::String(choices[next].clone()))
                .map_err(super::Error::from)
                .and_then(|candidate| apply_preferences(ui, settings, path, candidate));
              ui.options.error = result.err().map(|error| error.to_string());
            } else if row.key.starts_with("graph_symbol")
              || matches!(row.key, "disable_presets" | "log_level" | "temp_scale" | "apps_sort")
            {
              let choices = if row.key == "disable_presets" {
                &["off", "default", "custom", "all"][..]
              } else if row.key == "log_level" {
                &["OFF", "ERROR", "WARN", "INFO", "DEBUG", "TRACE"][..]
              } else if row.key == "temp_scale" {
                &["celsius", "fahrenheit", "kelvin", "rankine"][..]
              } else if row.key == "apps_sort" {
                &["cpu", "memory", "name", "state"][..]
              } else if row.key == "graph_symbol" {
                &["braille", "block", "tty"][..]
              } else {
                &["default", "braille", "block", "tty"][..]
              };
              let index = choices
                .iter()
                .position(|choice| values[row.key] == *choice)
                .unwrap_or(0);
              let result = settings
                .with_preference(
                  row.key,
                  Value::String(choices[(index + if previous { choices.len() - 1 } else { 1 }) % choices.len()].into()),
                )
                .map_err(super::Error::from)
                .and_then(|candidate| apply_preferences(ui, settings, path, candidate));
              ui.options.error = result.err().map(|error| error.to_string());
            } else if let Value::Bool(value) = values[row.key] {
              let result = settings
                .with_preference(row.key, Value::Bool(!value))
                .map_err(super::Error::from)
                .and_then(|candidate| apply_preferences(ui, settings, path, candidate));
              ui.options.error = result.err().map(|error| error.to_string());
            } else if matches!(key, KeyCode::Left | KeyCode::Right)
              && let Some(value) = values[row.key].as_i64()
            {
              let step = if row.key == "update_ms" { 100 } else { 1 };
              let value = if previous {
                value.saturating_sub(step)
              } else {
                value.saturating_add(step)
              };
              let result = settings
                .with_preference(row.key, Value::from(value))
                .map_err(super::Error::from)
                .and_then(|candidate| apply_preferences(ui, settings, path, candidate));
              ui.options.error = result.err().map(|error| error.to_string());
            } else if matches!(key, KeyCode::Enter | KeyCode::Char(' ')) {
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

pub(super) fn draw(frame: &mut Frame, ui: &mut Ui, settings: &Settings) {
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
  ui.hits.push((inner, super::mouse::Action::Scroll));
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
    ui.hits
      .push((Rect::new(x, inner.y, width, 1), super::mouse::Action::Category(index)));
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
  ui.options.page_size = visible.max(1);
  let start = ui.options.row / ui.options.page_size * ui.options.page_size;
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
    let value = if row.key == "theme"
      && let Some(choice) = &ui.options.theme_choice
    {
      format!(
        "{} ({}/{})",
        choice.names[choice.index],
        choice.index + 1,
        choice.names.len()
      )
    } else {
      text(&values[row.key])
    };
    let value = if value.is_empty() { "Default".into() } else { value };
    let value = if selected { format!("‹ {value} ›") } else { value };
    let lines = vec![Line::from(row.label), Line::from(value)];
    frame.render_widget(
      Paragraph::new(lines).centered().style(style),
      Rect::new(left.x, y, left.width, 2),
    );
    ui.hits
      .push((Rect::new(left.x, y, left.width, 2), super::mouse::Action::Option(index)));
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
    let footer = Rect::new(inner.x, inner.bottom() - 1, inner.width, 1);
    let hints = if ui.options.theme_choice.is_some() {
      super::mouse::hints(
        &mut ui.hits,
        footer,
        &[
          ("← prev", KeyCode::Left),
          ("next →", KeyCode::Right),
          ("Enter keep", KeyCode::Enter),
          ("Esc cancel", KeyCode::Esc),
        ],
      )
    } else {
      Line::from("↑↓ select · ←→ change · Enter edit · Esc back").centered()
    };
    frame.render_widget(
      Paragraph::new(hints).style(ui.theme.style().fg(ui.theme.color("inactive_fg"))),
      footer,
    );
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn it_changes_values_without_changing_category_and_pages_after_resizing() {
    use ratatui::{Terminal, backend::TestBackend};
    let mut ui = Ui::default();
    let mut settings = Settings::default();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    handle(&mut ui, &mut settings, &path, KeyCode::Right);
    assert!(!settings.save_on_exit());
    assert_eq!(ui.options.category, 0);
    ui.options.row = rows(0).iter().position(|row| row.key == "update_ms").unwrap();
    handle(&mut ui, &mut settings, &path, KeyCode::Left);
    assert_eq!(settings.update_ms(), 1900);
    handle(&mut ui, &mut settings, &path, KeyCode::Right);
    assert_eq!(settings.update_ms(), 2000);
    for height in [24, 40] {
      let mut terminal = Terminal::new(TestBackend::new(100, height)).unwrap();
      terminal.draw(|frame| draw(frame, &mut ui, &settings)).unwrap();
      let size = ui.options.page_size;
      handle(&mut ui, &mut settings, &path, KeyCode::Home);
      handle(&mut ui, &mut settings, &path, KeyCode::PageDown);
      assert_eq!(ui.options.row, size);
      handle(&mut ui, &mut settings, &path, KeyCode::PageUp);
      assert_eq!(ui.options.row, 0);
      handle(&mut ui, &mut settings, &path, KeyCode::Up);
      assert_eq!(ui.options.row, rows(0).len() - 1);
      handle(&mut ui, &mut settings, &path, KeyCode::Down);
      assert_eq!(ui.options.row, 0);
    }
    handle(&mut ui, &mut settings, &path, KeyCode::Char('5'));
    assert_eq!(ui.options.category, 4);
    handle(&mut ui, &mut settings, &path, KeyCode::Tab);
    assert_eq!(ui.options.category, 0);
  }

  #[tokio::test]
  async fn it_cycles_available_cpu_sources_and_saves_cpu_preferences() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    let mut settings = Settings::default();
    let before = settings.clone();
    let mut ui = Ui::default();
    let (_, receiver) = tokio::sync::watch::channel(crate::client::fixtures::state());
    ui.sessions.push(Some(super::super::Session {
      interval: tokio::sync::watch::channel(2000).0,
      receiver,
      task: tokio::spawn(std::future::pending()),
    }));
    ui.options.category = 1;
    ui.options.row = rows(1).iter().position(|row| row.key == "cpu_graph_upper").unwrap();
    for expected in ["total", "busiest", "cpu0", "cpu1", "cpu2", "cpu3", "auto", "total"] {
      handle(&mut ui, &mut settings, &path, KeyCode::Enter);
      assert_eq!(settings.cpu_graph_upper(), expected);
    }
    for key in [
      "cpu_invert_lower",
      "cpu_single_graph",
      "check_temp",
      "show_coretemp",
      "show_uptime",
    ] {
      ui.options.row = rows(1).iter().position(|row| row.key == key).unwrap();
      handle(&mut ui, &mut settings, &path, KeyCode::Enter);
    }
    crate::config::save_preferences(&path, &before, &settings).unwrap();
    let loaded = crate::config::load(&path).unwrap();
    assert_eq!(loaded.cpu_graph_upper(), "total");
    assert!(loaded.cpu_single_graph());
    assert!(!loaded.cpu_invert_lower());
    assert!(!loaded.check_temp());
    assert!(!loaded.show_coretemp());
    assert!(!loaded.show_uptime());
    assert!(
      settings
        .with_preference("cpu_graph_upper", Value::String("gpu".into()))
        .is_err()
    );
    assert!(
      settings
        .with_preference("temp_scale", Value::String("unknown".into()))
        .is_err()
    );
  }

  #[test]
  fn it_edits_general_preferences_and_preserves_vim_letters_in_the_clock() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    let mut settings = Settings::default();
    let before = settings.clone();
    let mut ui = Ui::default();
    ui.menu = Some(super::super::menu::Menu::Options);
    for key in [
      "rounded_corners",
      "tty_sync",
      "vim_keys",
      "base_10_sizes",
      "background_update",
    ] {
      ui.options.row = rows(0).iter().position(|row| row.key == key).unwrap();
      handle(&mut ui, &mut settings, &path, KeyCode::Enter);
    }
    ui.options.row = rows(0).iter().position(|row| row.key == "clock_format").unwrap();
    handle(&mut ui, &mut settings, &path, KeyCode::Enter);
    handle(&mut ui, &mut settings, &path, KeyCode::Home);
    for _ in 0..settings.clock_format().len() {
      handle(&mut ui, &mut settings, &path, KeyCode::Delete);
    }
    for letter in "hjkl".chars() {
      let key = super::super::navigation(&ui, &settings, KeyCode::Char(letter));
      handle(&mut ui, &mut settings, &path, key);
    }
    handle(&mut ui, &mut settings, &path, KeyCode::Enter);
    assert_eq!(settings.clock_format(), "hjkl");
    ui.options.row = rows(0).iter().position(|row| row.key == "log_level").unwrap();
    handle(&mut ui, &mut settings, &path, KeyCode::Enter);
    assert_eq!(settings.log_level(), log::LevelFilter::Info);
    crate::config::save_preferences(&path, &before, &settings).unwrap();
    let loaded = crate::config::load(&path).unwrap();
    assert!(!loaded.rounded_corners());
    assert!(!loaded.tty_sync());
    assert!(loaded.vim_keys());
    assert!(loaded.base_10_sizes());
    assert!(!loaded.background_update());
    assert_eq!(loaded.log_level(), log::LevelFilter::Info);
    assert_eq!(loaded.clock_format(), "hjkl");
  }

  #[test]
  fn it_cycles_graph_choices_and_inherits_the_global_style() {
    use crate::config::GraphSymbol;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    let mut settings = Settings::default();
    let before = settings.clone();
    let mut ui = Ui::default();
    ui.options.row = rows(0).iter().position(|row| row.key == "graph_symbol").unwrap();
    handle(&mut ui, &mut settings, &path, KeyCode::Enter);
    assert_eq!(settings.graph_symbol(), GraphSymbol::Block);
    assert_eq!(
      settings.graph_symbol_cpu().resolve(settings.graph_symbol()),
      GraphSymbol::Block
    );
    handle(&mut ui, &mut settings, &path, KeyCode::Tab);
    handle(&mut ui, &mut settings, &path, KeyCode::Enter);
    assert_eq!(
      settings.graph_symbol_cpu().resolve(settings.graph_symbol()),
      GraphSymbol::Braille
    );
    crate::config::save_preferences(&path, &before, &settings).unwrap();
    let loaded = crate::config::load(&path).unwrap();
    assert_eq!(loaded.graph_symbol(), GraphSymbol::Block);
    assert_eq!(loaded.graph_symbol_cpu(), GraphSymbol::Braille);
    assert!(
      settings
        .with_preference("graph_symbol", Value::String("default".into()))
        .is_err()
    );
    assert!(
      settings
        .with_preference("graph_symbol_cpu", Value::String("unknown".into()))
        .is_err()
    );
  }

  #[tokio::test]
  async fn it_previews_cancels_and_saves_themes_without_replacing_sessions() {
    use ratatui::style::Color;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    std::fs::create_dir(directory.path().join("themes")).unwrap();
    std::fs::write(
      directory.path().join("themes/0-preview.theme"),
      "theme[main_bg]=\"#010203\"",
    )
    .unwrap();
    std::fs::write(directory.path().join("themes/zz-broken.theme"), "bad theme").unwrap();
    let mut settings = Settings::default();
    let before = settings.clone();
    let mut ui = Ui::default();
    let (_, receiver) = tokio::sync::watch::channel(crate::client::State::default());
    let task = tokio::spawn(std::future::pending());
    let id = task.id();
    ui.sessions.push(Some(super::super::Session {
      interval: tokio::sync::watch::channel(2000).0,
      receiver,
      task,
    }));
    ui.options.row = 1;
    handle(&mut ui, &mut settings, &path, KeyCode::Enter);
    handle(&mut ui, &mut settings, &path, KeyCode::Right);
    assert!(settings.theme().is_none());
    let preview = ui.theme.color("main_bg");
    assert_ne!(preview, Color::Rgb(0, 0, 0));
    handle(&mut ui, &mut settings, &path, KeyCode::End);
    assert!(ui.options.error.is_some());
    assert_eq!(ui.theme.color("main_bg"), preview);
    handle(&mut ui, &mut settings, &path, KeyCode::Enter);
    assert!(ui.options.theme_choice.is_some());
    assert!(settings.theme().is_none());
    handle(&mut ui, &mut settings, &path, KeyCode::Esc);
    assert_eq!(ui.theme.color("main_bg"), Color::Rgb(0, 0, 0));
    handle(&mut ui, &mut settings, &path, KeyCode::Enter);
    handle(&mut ui, &mut settings, &path, KeyCode::Right);
    handle(&mut ui, &mut settings, &path, KeyCode::Enter);
    assert!(ui.options.theme_choice.is_none());
    assert!(settings.theme().is_some());
    crate::config::save_preferences(&path, &before, &settings).unwrap();
    assert_eq!(crate::config::load(&path).unwrap().theme(), settings.theme());
    ui.options.row = 2;
    handle(&mut ui, &mut settings, &path, KeyCode::Enter);
    assert!(!settings.theme_background());
    assert_eq!(ui.theme.color("main_bg"), Color::Reset);
    handle(&mut ui, &mut settings, &path, KeyCode::Enter);
    assert_eq!(ui.theme.color("main_bg"), preview);
    assert_eq!(ui.sessions[0].as_ref().unwrap().task.id(), id);
  }

  #[test]
  fn it_keeps_values_below_names_and_descriptions_on_the_right() {
    use ratatui::{Terminal, backend::TestBackend};
    let mut ui = Ui::default();
    for (width, height) in [(1, 1), (20, 4), (48, 16), (80, 24), (160, 60)] {
      let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
      terminal
        .draw(|frame| draw(frame, &mut ui, &Settings::default()))
        .unwrap();
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
    ui.options.row = rows(0).iter().position(|row| row.key == "update_ms").unwrap();
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
