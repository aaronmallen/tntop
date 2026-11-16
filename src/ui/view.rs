mod apps;
mod cpu;
mod memory;
mod network;

use ratatui::{
  Frame,
  layout::{Alignment, Constraint, Layout, Rect},
  style::{Color, Modifier, Style},
  text::{Line, Span},
  widgets::{Block, BorderType, Paragraph, Row, Table, TableState, Wrap},
};

use super::{Ui, theme::Theme};
use crate::config::Settings;

pub(super) fn draw(frame: &mut Frame, ui: &mut Ui, settings: &Settings) {
  ui.hits.clear();
  draw_content(frame, ui, settings);
  if ui.menu.is_some() {
    let area = frame.area();
    frame
      .buffer_mut()
      .set_style(area, ui.theme.style().fg(ui.theme.color("inactive_fg")));
  }
  if ui.menu.is_some() || ui.dialog.is_some() || ui.discovery.is_some() {
    ui.hits.clear();
  }
  if ui.menu == Some(super::menu::Menu::Options) {
    super::options::draw(frame, ui, settings);
  } else {
    super::menu::draw(frame, ui);
  }
}

fn draw_content(frame: &mut Frame, ui: &mut Ui, settings: &Settings) {
  let theme = &ui.theme;
  frame.render_widget(Block::default().style(theme.style()), frame.area());
  if frame.area().width < 70 || frame.area().height < 24 {
    frame.render_widget(
      Paragraph::new("tntop needs at least 70 columns × 24 rows.\nResize the terminal, or press q to quit.").wrap(
        Wrap {
          trim: true,
        },
      ),
      frame.area(),
    );
    return;
  }
  if ui.picker {
    picker(frame, ui, settings);
    super::manager::draw(frame, ui, settings);
    return;
  }
  let state = ui.sessions[ui.selected]
    .as_ref()
    .map(|session| session.receiver.borrow().clone())
    .unwrap_or_default();
  let [header, cpu, bottom, footer] = Layout::vertical([
    Constraint::Length(1),
    Constraint::Percentage(32),
    Constraint::Min(13),
    Constraint::Length(2),
  ])
  .areas(frame.area());
  let [heading, clock] = Layout::horizontal([Constraint::Min(1), Constraint::Length(13)]).areas(header);
  frame.render_widget(
    Paragraph::new(Line::from(vec![
      Span::styled(
        " tntop ",
        Style::default()
          .fg(Color::Rgb(145, 24, 254))
          .add_modifier(Modifier::BOLD),
      ),
      Span::raw(format!(
        " {}  {}  {}ms",
        settings.servers()[ui.selected].name(),
        state.version(),
        settings.update_ms()
      )),
    ])),
    heading,
  );
  let seconds = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs();
  frame.render_widget(
    Paragraph::new(format!(
      "{:02}:{:02}:{:02} UTC",
      seconds / 3600 % 24,
      seconds / 60 % 60,
      seconds % 60
    ))
    .alignment(Alignment::Right),
    clock,
  );
  cpu::draw(frame, cpu, &state, theme);
  let [left, apps] = Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)]).areas(bottom);
  let [memory, network] = Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)]).areas(left);
  memory::draw(frame, memory, &state, ui.pool, theme);
  network::draw(frame, network, &state, ui.interface, theme);
  apps::draw(frame, apps, &state, ui);
  let status = match state.error() {
    Some(error) => format!(" {error} | last sample {}", age(*state.updated_at())),
    None => format!(
      " sample {} | API {}s | network totals: session estimates | GPU unavailable",
      age(*state.updated_at()),
      settings.update_ms().div_ceil(1000).max(2)
    ),
  };
  let hints = super::mouse::hints(
    &mut ui.hits,
    footer,
    &[
      ("m menu", ratatui::crossterm::event::KeyCode::Char('m')),
      ("s servers", ratatui::crossterm::event::KeyCode::Char('s')),
      ("/ filter", ratatui::crossterm::event::KeyCode::Char('/')),
      ("← sort", ratatui::crossterm::event::KeyCode::Left),
      ("sort →", ratatui::crossterm::event::KeyCode::Right),
      ("n net", ratatui::crossterm::event::KeyCode::Char('n')),
      ("[ pool", ratatui::crossterm::event::KeyCode::Char('[')),
      ("pool ]", ratatui::crossterm::event::KeyCode::Char(']')),
    ],
  );
  frame.render_widget(
    Paragraph::new(vec![
      hints,
      Line::styled(
        ui.message.clone().unwrap_or(status),
        Style::default().fg(ui.theme.color("graph_text")),
      ),
    ]),
    footer,
  );
}

pub(super) fn panel<'a>(title: &str, color: &str, theme: &Theme) -> Block<'a> {
  Block::bordered()
    .border_type(BorderType::Rounded)
    .border_style(Style::default().fg(theme.color(color)))
    .title(Span::styled(
      title.to_owned(),
      Style::default().fg(theme.color("title")).add_modifier(Modifier::BOLD),
    ))
}

fn age(instant: Option<std::time::Instant>) -> String {
  instant
    .map(|time| format!("{:.1}s ago", time.elapsed().as_secs_f64()))
    .unwrap_or_else(|| "waiting".into())
}

fn bytes(mut value: f64) -> String {
  for unit in ["B", "KiB", "MiB", "GiB", "TiB", "PiB"] {
    if value < 1024.0 || unit == "PiB" {
      return format!("{value:.1} {unit}");
    }
    value /= 1024.0;
  }
  unreachable!()
}

fn graph(frame: &mut Frame, area: Rect, values: &[f64], maximum: f64, gradient: &str, theme: &Theme, inverted: bool) {
  let capacity = area.width as usize * 2;
  let start = values.len().saturating_sub(capacity);
  let offset = capacity.saturating_sub(values.len());
  for x in 0..area.width {
    for y in 0..area.height {
      let mut bits = 0u32;
      for dx in 0..2 {
        let index = x as usize * 2 + dx;
        let value = index
          .checked_sub(offset)
          .and_then(|index| values.get(start + index))
          .copied()
          .unwrap_or(0.0);
        let height = (value / maximum.max(1.0)).clamp(0.0, 1.0) * area.height as f64 * 4.0;
        for dy in 0..4 {
          let pixel = if inverted {
            y as usize * 4 + dy
          } else {
            (area.height - y - 1) as usize * 4 + (3 - dy)
          };
          if (pixel as f64) < height {
            bits |= 1 << [[0, 1, 2, 6], [3, 4, 5, 7]][dx][dy];
          }
        }
      }
      let ratio = if inverted {
        y as f64 / area.height.max(1) as f64
      } else {
        1.0 - y as f64 / area.height.max(1) as f64
      };
      frame.buffer_mut()[(area.x + x, area.y + y)]
        .set_char(if bits == 0 {
          ' '
        } else {
          char::from_u32(0x2800 + bits).unwrap_or(' ')
        })
        .set_fg(theme.gradient(gradient, ratio));
    }
  }
}

fn meter(frame: &mut Frame, area: Rect, label: &str, ratio: f64, value: &str, gradient: &str, theme: &Theme) {
  let width = (area.width as usize).saturating_sub(label.chars().count() + value.chars().count() + 4);
  let filled = (ratio.clamp(0.0, 1.0) * width as f64).round() as usize;
  let mut spans = vec![Span::raw(format!(" {label} "))];
  for i in 0..width {
    spans.push(Span::styled(
      "■",
      Style::default().fg(if i < filled {
        theme.gradient(gradient, i as f64 / width.max(1) as f64)
      } else {
        theme.color("meter_bg")
      }),
    ));
  }
  spans.push(Span::raw(format!(" {value}")));
  frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn picker(frame: &mut Frame, ui: &mut Ui, settings: &Settings) {
  let [header, list, footer] =
    Layout::vertical([Constraint::Length(3), Constraint::Min(1), Constraint::Length(3)]).areas(frame.area());
  frame.render_widget(
    Paragraph::new(" tntop\n Server manager: select a TrueNAS server").style(
      Style::default()
        .fg(ui.theme.color("title"))
        .add_modifier(Modifier::BOLD),
    ),
    header,
  );
  let rows = settings.servers().iter().enumerate().map(|(index, server)| {
    let status = match &ui.sessions[index] {
      None => "not connected",
      Some(session) if session.receiver.borrow().error().is_some() => "disconnected",
      Some(session) if session.receiver.borrow().updated_at().is_some() => "monitoring",
      Some(_) => "connecting",
    };
    Row::new([server.name().as_str(), server.url().as_str(), status])
  });
  let table = Table::new(
    rows,
    [
      Constraint::Percentage(25),
      Constraint::Percentage(55),
      Constraint::Percentage(20),
    ],
  )
  .block(panel(" servers ", "cpu_box", &ui.theme))
  .header(Row::new(["NAME", "ADDRESS", "STATUS"]))
  .row_highlight_style(
    Style::default()
      .fg(ui.theme.color("selected_fg"))
      .bg(ui.theme.color("selected_bg")),
  )
  .highlight_symbol("› ");
  let mut table_state = TableState::default().with_selected(Some(ui.selected));
  frame.render_stateful_widget(table, list, &mut table_state);
  super::mouse::rows(
    &mut ui.hits,
    Rect::new(
      list.x + 1,
      list.y + 2,
      list.width.saturating_sub(2),
      list.height.saturating_sub(3),
    ),
    table_state.offset(),
    settings.servers().len(),
    true,
  );
  let hints = super::mouse::hints(
    &mut ui.hits,
    footer,
    &[
      ("Enter open", ratatui::crossterm::event::KeyCode::Enter),
      ("a add", ratatui::crossterm::event::KeyCode::Char('a')),
      ("d delete", ratatui::crossterm::event::KeyCode::Char('d')),
      ("t trust", ratatui::crossterm::event::KeyCode::Char('t')),
      ("e API key", ratatui::crossterm::event::KeyCode::Char('e')),
      ("m menu", ratatui::crossterm::event::KeyCode::Char('m')),
    ],
  );
  frame.render_widget(
    Paragraph::new(vec![
      hints,
      Line::from(
        ui.message
          .as_deref()
          .unwrap_or(if settings.servers().is_empty() {
            "No servers configured. Press a to add your first server."
          } else {
            "API keys live in your system keyring."
          })
          .to_owned(),
      ),
    ])
    .wrap(Wrap {
      trim: true,
    }),
    footer,
  );
}

fn sparkline(values: impl IntoIterator<Item = f64>, width: usize) -> String {
  let values: Vec<_> = values.into_iter().collect();
  let mut line = " ".repeat(width.saturating_sub(values.len()));
  for value in values.iter().skip(values.len().saturating_sub(width)) {
    line.push([' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'][(value.clamp(0.0, 100.0) * 0.08).ceil() as usize]);
  }
  line
}

#[cfg(test)]
mod tests {
  use ratatui::{Terminal, backend::TestBackend};
  use tokio::sync::watch;

  use super::*;
  use crate::{client::fixtures, ui::Session};

  fn settings() -> Settings {
    serde_json::from_value(serde_json::json!({"servers":[{"name":"Storage","url":"https://nas.example.com"}]})).unwrap()
  }

  fn text(terminal: &Terminal<TestBackend>) -> String {
    terminal
      .backend()
      .buffer()
      .content
      .chunks(terminal.backend().buffer().area.width as usize)
      .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
      .collect::<Vec<_>>()
      .join("\n")
  }

  fn ui() -> Ui {
    Ui {
      app_row: 0,
      dialog: None,
      discovery: None,
      filter: String::new(),
      filtering: false,
      interface: 0,
      message: None,
      menu: None,
      options: Default::default(),
      hits: Vec::new(),
      picker: false,
      pool: 0,
      selected: 0,
      sessions: vec![None],
      sort: 0,
      theme: Theme::default(),
      update: Default::default(),
    }
  }

  mod apps {
    use super::*;

    mod draw {
      use super::*;

      #[test]
      fn it_filters_and_sorts_apps_without_losing_update_status() {
        let mut terminal = Terminal::new(TestBackend::new(120, 25)).unwrap();
        let mut ui = ui();
        let state = fixtures::state();
        ui.filter = "forge".into();
        terminal
          .draw(|frame| super::super::super::apps::draw(frame, frame.area(), &state, &mut ui))
          .unwrap();
        let output = text(&terminal);
        assert!(output.contains("forgejo"));
        assert!(output.contains("update: available"));
        assert!(!output.contains("postgres"));
        ui.filter.clear();
        ui.sort = 2;
        ui.app_row = 1;
        terminal
          .draw(|frame| super::super::super::apps::draw(frame, frame.area(), &state, &mut ui))
          .unwrap();
        assert!(text(&terminal).contains("jellyfin  10.11"));
      }

      #[test]
      fn it_marks_denied_stats_as_unavailable_instead_of_zero() {
        let state = fixtures::without_app_stats();
        let mut terminal = Terminal::new(TestBackend::new(120, 25)).unwrap();
        let mut ui = ui();
        terminal
          .draw(|frame| super::super::super::apps::draw(frame, frame.area(), &state, &mut ui))
          .unwrap();
        assert!(text(&terminal).contains("N/A"));
        assert!(text(&terminal).contains("APPS_READ"));
      }
    }
  }

  mod draw {
    use super::*;

    #[tokio::test]
    async fn it_renders_live_metrics_at_compact_and_reference_sizes() {
      let state = fixtures::state();
      for (width, height) in [(70, 24), (100, 32), (160, 48), (280, 85)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let mut ui = ui();
        let (_sender, receiver) = watch::channel(state.clone());
        ui.sessions[0] = Some(Session {
          receiver,
          task: tokio::spawn(std::future::pending()),
        });
        terminal
          .draw(|frame| super::super::draw(frame, &mut ui, &settings()))
          .unwrap();
        let output = text(&terminal);
        for expected in ["apps", "cpu", "net", "mem / zfs", "forgejo"] {
          assert!(
            output.contains(expected),
            "missing {expected} at {width}x{height}\n{output}"
          );
        }
        if width >= 160 {
          for expected in [
            "Intel(R) N150",
            "Load avg:",
            "busiest core",
            "ZFS free",
            "storage_share",
            "ARC hit 99.0%",
            "Session ~",
            "Disk totals R",
            "CPU%",
            "MEMORY",
          ] {
            assert!(
              output.contains(expected),
              "missing {expected} at {width}x{height}\n{output}"
            );
          }
          let graphs = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .filter(|cell| cell.symbol().chars().any(|c| ('\u{2801}'..='\u{28ff}').contains(&c)))
            .count();
          assert!(graphs > 100, "expected CPU, memory, disk and network history graphs");
        }
      }
    }

    #[test]
    fn it_renders_the_picker_and_small_terminal_message() {
      for (width, height, picker, expected) in [
        (100, 32, true, "Server manager"),
        (40, 10, false, "Resize the terminal"),
      ] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let mut ui = ui();
        ui.picker = picker;
        terminal
          .draw(|frame| super::super::draw(frame, &mut ui, &settings()))
          .unwrap();
        assert!(text(&terminal).contains(expected));
      }
    }
  }

  mod graph {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_right_aligns_samples_and_mirrors_uploads() {
      let mut terminal = Terminal::new(TestBackend::new(4, 2)).unwrap();
      for inverted in [false, true] {
        terminal
          .draw(|frame| {
            super::super::graph(
              frame,
              frame.area(),
              &[25.0, 25.0],
              100.0,
              "cpu",
              &Theme::default(),
              inverted,
            )
          })
          .unwrap();
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(0, 0)].symbol(), " ");
        let (filled, empty) = if inverted { (0, 1) } else { (1, 0) };
        assert_ne!(buffer[(3, filled)].symbol(), " ");
        assert_eq!(buffer[(3, empty)].symbol(), " ");
      }
    }
  }
}
