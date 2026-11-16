mod apps;
mod cpu;
mod memory;
mod network;

use ratatui::{
  Frame,
  layout::{Alignment, Constraint, Layout, Rect},
  style::{Color, Modifier, Style},
  text::{Line, Span},
  widgets::{Block, BorderType, Paragraph, Row, Table, TableState, Widget, Wrap},
};

use super::{Ui, theme::Theme};
use crate::config::{GraphSymbol, Settings};

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
  super::display::apply(frame, settings.truecolor(), ui.theme.is_tty());
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
  let [header, body, footer] =
    Layout::vertical([Constraint::Length(1), Constraint::Min(0), Constraint::Length(2)]).areas(frame.area());
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
  let tty = theme.is_tty();
  let symbol = |symbol: GraphSymbol| {
    if tty {
      GraphSymbol::Tty
    } else {
      symbol.resolve(settings.graph_symbol())
    }
  };
  let [cpu, memory, network, apps] = super::layout::areas(body, settings);
  if !cpu.is_empty() {
    cpu::draw(frame, cpu, &state, theme, symbol(settings.graph_symbol_cpu()));
  }
  if !memory.is_empty() {
    memory::draw(
      frame,
      memory,
      &state,
      ui.pool,
      theme,
      symbol(settings.graph_symbol_mem()),
    );
  }
  if !network.is_empty() {
    network::draw(
      frame,
      network,
      &state,
      ui.interface,
      theme,
      symbol(settings.graph_symbol_net()),
    );
  }
  if !apps.is_empty() {
    apps::draw(frame, apps, &state, ui, symbol(settings.graph_symbol_apps()));
  }
  if [cpu, memory, network, apps].into_iter().all(Rect::is_empty) {
    frame.render_widget(
      Paragraph::new("No panels selected. Press 1–4 to show panels, or m for the menu.")
        .centered()
        .wrap(Wrap {
          trim: true,
        }),
      body,
    );
  }
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
      ("1 cpu", ratatui::crossterm::event::KeyCode::Char('1')),
      ("2 mem", ratatui::crossterm::event::KeyCode::Char('2')),
      ("3 net", ratatui::crossterm::event::KeyCode::Char('3')),
      ("4 apps", ratatui::crossterm::event::KeyCode::Char('4')),
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

struct Graph<'a> {
  values: &'a [f64],
  maximum: f64,
  gradient: &'a str,
  theme: &'a Theme,
  inverted: bool,
  symbol: GraphSymbol,
}

impl Graph<'_> {
  fn cell(&self, width: u16, height: u16, x: u16, y: u16) -> char {
    let Self {
      values,
      maximum,
      inverted,
      symbol,
      ..
    } = self;
    let columns = symbol.samples_per_cell();
    let rows = match symbol {
      GraphSymbol::Tty => 1,
      GraphSymbol::Block => 2,
      _ => 4,
    };
    let capacity = width as usize * columns;
    let start = values.len().saturating_sub(capacity);
    let offset = capacity.saturating_sub(values.len());
    let mut bits = 0u32;
    for dx in 0..columns {
      let index = x as usize * columns + dx;
      let value = index
        .checked_sub(offset)
        .and_then(|index| values.get(start + index))
        .copied()
        .unwrap_or(0.0);
      let filled = (value / maximum.max(1.0)).clamp(0.0, 1.0) * height as f64 * rows as f64;
      for dy in 0..rows {
        let pixel = if *inverted {
          y as usize * rows + dy
        } else {
          (height - y - 1) as usize * rows + (rows - 1 - dy)
        };
        if (pixel as f64) < filled {
          let bit = if matches!(symbol, GraphSymbol::Braille | GraphSymbol::Default) {
            [[0, 1, 2, 6], [3, 4, 5, 7]][dx][dy]
          } else {
            dx * rows + dy
          };
          bits |= 1 << bit;
        }
      }
    }
    match symbol {
      GraphSymbol::Tty => {
        if bits == 0 {
          ' '
        } else {
          '#'
        }
      }
      GraphSymbol::Block => [
        ' ', '▘', '▖', '▌', '▝', '▀', '▞', '▛', '▗', '▚', '▄', '▙', '▐', '▜', '▟', '█',
      ][bits as usize],
      _ => {
        if bits == 0 {
          ' '
        } else {
          char::from_u32(0x2800 + bits).unwrap_or(' ')
        }
      }
    }
  }
}

impl Widget for Graph<'_> {
  fn render(self, area: Rect, buffer: &mut ratatui::buffer::Buffer) {
    for x in 0..area.width {
      for y in 0..area.height {
        let ratio = if self.inverted {
          y as f64 / area.height.max(1) as f64
        } else {
          1.0 - y as f64 / area.height.max(1) as f64
        };
        buffer[(area.x + x, area.y + y)]
          .set_char(self.cell(area.width, area.height, x, y))
          .set_fg(self.theme.gradient(self.gradient, ratio));
      }
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

fn sparkline(values: impl IntoIterator<Item = f64>, width: u16, symbol: GraphSymbol, theme: &Theme) -> String {
  let values: Vec<_> = values.into_iter().collect();
  let graph = Graph {
    values: &values,
    maximum: 100.0,
    gradient: "cpu",
    theme,
    inverted: false,
    symbol,
  };
  (0..width).map(|x| graph.cell(width, 1, x, 0)).collect()
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
          .draw(|frame| super::super::super::apps::draw(frame, frame.area(), &state, &mut ui, GraphSymbol::Braille))
          .unwrap();
        let output = text(&terminal);
        assert!(output.contains("forgejo"));
        assert!(output.contains("update: available"));
        assert!(!output.contains("postgres"));
        ui.filter.clear();
        ui.sort = 2;
        ui.app_row = 1;
        terminal
          .draw(|frame| super::super::super::apps::draw(frame, frame.area(), &state, &mut ui, GraphSymbol::Braille))
          .unwrap();
        assert!(text(&terminal).contains("jellyfin  10.11"));
      }

      #[test]
      fn it_marks_denied_stats_as_unavailable_instead_of_zero() {
        let state = fixtures::without_app_stats();
        let mut terminal = Terminal::new(TestBackend::new(120, 25)).unwrap();
        let mut ui = ui();
        terminal
          .draw(|frame| super::super::super::apps::draw(frame, frame.area(), &state, &mut ui, GraphSymbol::Braille))
          .unwrap();
        assert!(text(&terminal).contains("N/A"));
        assert!(text(&terminal).contains("APPS_READ"));
      }
    }
  }

  mod draw {
    use super::*;

    #[tokio::test]
    async fn it_toggles_all_panels_and_resizes_without_replacing_the_session() {
      let mut settings = settings();
      let mut ui = ui();
      let (_, receiver) = watch::channel(fixtures::state());
      ui.sessions[0] = Some(Session {
        receiver,
        task: tokio::spawn(std::future::pending()),
      });
      let id = ui.sessions[0].as_ref().unwrap().task.id();
      let history = ui.sessions[0].as_ref().unwrap().receiver.borrow().history().len();
      for mask in 0..16 {
        for (index, key) in ["show_cpu", "show_mem", "show_net", "show_apps"].iter().enumerate() {
          let current = settings.preferences().unwrap()[key].as_bool().unwrap();
          if current != (mask & (1 << index) != 0) {
            assert!(crate::ui::layout::handle(
              &mut ui,
              &mut settings,
              std::path::Path::new("config.toml"),
              ratatui::crossterm::event::KeyCode::Char(char::from(b'1' + index as u8))
            ));
          }
        }
        for (width, height) in [(20, 4), (70, 24), (160, 48)] {
          let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
          terminal
            .draw(|frame| super::super::draw(frame, &mut ui, &settings))
            .unwrap();
          if width >= 70 && mask == 0 {
            assert!(text(&terminal).contains("No panels selected"));
          }
        }
      }
      assert_eq!(ui.sessions[0].as_ref().unwrap().task.id(), id);
      assert_eq!(
        ui.sessions[0].as_ref().unwrap().receiver.borrow().history().len(),
        history
      );
      assert_eq!(ui.selected, 0);
    }

    #[tokio::test]
    async fn it_applies_display_modes_to_panels_and_every_overlay_then_restores_choices() {
      use std::path::Path;

      use crate::ui::{apply_preferences, load_theme, manager::Dialog, menu::Menu};
      let path = Path::new("config.toml");
      let mut settings = settings()
        .with_preference("theme", serde_json::json!("dracula"))
        .unwrap();
      let mut ui = ui();
      let (_, receiver) = watch::channel(fixtures::state());
      ui.sessions[0] = Some(Session {
        receiver,
        task: tokio::spawn(std::future::pending()),
      });
      let id = ui.sessions[0].as_ref().unwrap().task.id();
      let original = load_theme(&settings, path).unwrap().color("main_bg");
      for tty in [false, true] {
        let candidate = settings
          .with_preference("truecolor", serde_json::json!(false))
          .unwrap()
          .with_preference("force_tty", serde_json::json!(tty))
          .unwrap();
        apply_preferences(&mut ui, &mut settings, path, candidate).unwrap();
        for layer in 0..7 {
          ui.menu = match layer {
            1 => Some(Menu::Main(0)),
            2 => Some(Menu::Options),
            3 => Some(Menu::Help(0)),
            4 => Some(Menu::Update),
            _ => None,
          };
          ui.picker = layer >= 5;
          ui.dialog = (layer == 6).then_some(Dialog::Add {
            field: 0,
            values: Default::default(),
          });
          let mut terminal = Terminal::new(TestBackend::new(160, 48)).unwrap();
          terminal
            .draw(|frame| super::super::draw(frame, &mut ui, &settings))
            .unwrap();
          for cell in &terminal.backend().buffer().content {
            assert!(!matches!(cell.fg, Color::Rgb(..)));
            assert!(!matches!(cell.bg, Color::Rgb(..)));
            if tty {
              assert!(cell.symbol().is_ascii(), "{}", cell.symbol());
              assert!(!matches!(cell.fg, Color::Indexed(16..)));
              assert!(!matches!(cell.bg, Color::Indexed(16..)));
            }
          }
        }
      }
      let candidate = settings.with_preference("force_tty", serde_json::json!(false)).unwrap();
      apply_preferences(&mut ui, &mut settings, path, candidate).unwrap();
      assert_eq!(ui.theme.color("main_bg"), original);
      assert_eq!(settings.theme().as_deref(), Some(Path::new("dracula")));
      assert_eq!(settings.graph_symbol(), GraphSymbol::Braille);
      assert_eq!(ui.sessions[0].as_ref().unwrap().task.id(), id);
    }

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
      for symbol in [GraphSymbol::Braille, GraphSymbol::Block, GraphSymbol::Tty] {
        for inverted in [false, true] {
          terminal
            .draw(|frame| {
              frame.render_widget(
                Graph {
                  values: &[25.0, 25.0],
                  maximum: 100.0,
                  gradient: "cpu",
                  theme: &Theme::default(),
                  inverted,
                  symbol,
                },
                frame.area(),
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

    #[test]
    fn it_uses_each_symbols_capacity_and_keeps_history_when_switching() {
      let theme = Theme::default();
      let values = [100.0, 0.0, 0.0, 0.0];
      for symbol in [
        GraphSymbol::Braille,
        GraphSymbol::Block,
        GraphSymbol::Tty,
        GraphSymbol::Braille,
      ] {
        let line = sparkline(values, 2, symbol, &theme);
        assert_eq!(line.chars().count(), 2);
        assert_eq!(line == "  ", symbol == GraphSymbol::Tty);
        assert_eq!(sparkline([f64::NAN, 0.0], 2, symbol, &theme), "  ");
        assert_eq!(sparkline([], 0, symbol, &theme), "");
        for (width, height) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
          let area = Rect::new(0, 0, width, height);
          let mut buffer = ratatui::buffer::Buffer::empty(area);
          Graph {
            values: &[],
            maximum: 0.0,
            gradient: "cpu",
            theme: &theme,
            inverted: true,
            symbol,
          }
          .render(area, &mut buffer);
          assert!(buffer.content.iter().all(|cell| cell.symbol() == " "));
        }
      }
    }
  }
}
