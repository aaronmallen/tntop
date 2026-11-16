use ratatui::widgets::Cell;

use super::*;
use crate::client::State;

pub(super) fn draw(frame: &mut Frame, area: Rect, state: &State, ui: &mut Ui, settings: &Settings) {
  let state = ui.app_paused.as_ref().unwrap_or(state);
  let theme = &ui.theme;
  let symbol = graph_symbol(settings, theme, settings.graph_symbol_apps());
  let bytes = |value| super::bytes(value, settings.base_10_sizes());
  let title = format!(" 4 apps{} ", if ui.app_paused.is_some() { " PAUSED" } else { "" },);
  let block = panel(&title, "proc_box", theme);
  let inner = block.inner(area);
  frame.render_widget(block, area);
  let title_width = Line::from(title).width() as u16 + 1;
  super::super::mouse::controls(
    frame,
    &mut ui.hits,
    Rect::new(
      area.x.saturating_add(title_width),
      area.y,
      area.width.saturating_sub(title_width + 1),
      u16::from(area.height > 1),
    ),
    theme,
    &[
      ("", "←", "", KeyCode::Left),
      ("", "→", settings.apps_sort(), KeyCode::Right),
      (
        "",
        "/",
        &format!(
          " {}{}",
          if ui.filter.is_empty() { "filter" } else { &ui.filter },
          if ui.filtering { "▏" } else { "" }
        ),
        KeyCode::Char('/'),
      ),
      (
        "",
        "r",
        if settings.apps_reversed() {
          "everse on"
        } else {
          "everse"
        },
        KeyCode::Char('r'),
      ),
    ],
  );
  super::super::mouse::controls(
    frame,
    &mut ui.hits,
    Rect::new(
      inner.x,
      area.bottom().saturating_sub(1),
      inner.width,
      u16::from(area.height > 1),
    ),
    theme,
    &[
      (
        "",
        "enter",
        if inner.width >= 40 { " details" } else { "" },
        KeyCode::Enter,
      ),
      (
        "",
        "f",
        if ui.app_follow.is_some() { "ollow on" } else { "ollow" },
        KeyCode::Char('f'),
      ),
      (
        "",
        "space",
        if ui.app_paused.is_some() { " resume" } else { " pause" },
        KeyCode::Char(' '),
      ),
    ],
  );
  let Some(all_apps) = state.apps() else {
    frame.render_widget(
      Paragraph::new(state.apps_error().as_deref().unwrap_or("Waiting for installed apps…")).wrap(Wrap {
        trim: true,
      }),
      inner,
    );
    return;
  };
  let [table_area, detail, status] = Layout::vertical([
    Constraint::Min(1),
    Constraint::Length(if ui.app_details { 5 } else { 0 }),
    Constraint::Length(1),
  ])
  .areas(inner);
  ui.app_page = table_area.height.saturating_sub(1).max(1) as usize;
  let stats = state.app_history().back();
  let apps = super::super::apps::ordered(state, &ui.filter, settings);
  if let Some(selected) = &ui.app_selected {
    if let Some(index) = apps.iter().position(|app| app.name() == selected) {
      ui.app_row = index;
    } else if !all_apps.iter().any(|app| app.name() == selected) {
      ui.app_selected = None;
    } else {
      ui.app_row = 0;
    }
  }
  ui.app_row = ui.app_row.min(apps.len().saturating_sub(1));
  if ui.app_selected.is_none() {
    ui.app_selected = apps.get(ui.app_row).map(|app| app.name().clone());
  }
  let wide = inner.width >= 85;
  let compact = inner.width < 55;
  let rows = apps.iter().map(|app| {
    let stats = stats.and_then(|stats| stats.get(app.name()));
    let color = if !settings.apps_colors() {
      theme.color("main_fg")
    } else {
      match app.state().as_str() {
        "RUNNING" => theme.gradient("cpu", 0.0),
        "CRASHED" => theme.gradient("cpu", 1.0),
        "STOPPED" => theme.color("inactive_fg"),
        _ => theme.gradient("cpu", 0.5),
      }
    };
    let cpu = stats
      .and_then(|stats| super::super::apps::cpu(*stats.cpu_usage(), state, settings))
      .map(|cpu| format!("{cpu:5.1}"))
      .unwrap_or_else(|| "  N/A".into());
    let memory = stats
      .and_then(|stats| {
        if settings.apps_mem_bytes() {
          Some(bytes(*stats.memory() as f64))
        } else {
          state.history().back().map(|sample| {
            format!(
              "{:.1}%",
              *stats.memory() as f64 / sample.memory().physical_memory_total() as f64 * 100.0
            )
          })
        }
      })
      .unwrap_or_else(|| "N/A".into());
    let mut cells = vec![
      Cell::from(app.name().as_str()),
      Cell::from(app.state().as_str()).style(Style::default().fg(color)),
      Cell::from(if app.update() == "available" { "YES" } else { "-" })
        .style(Style::default().fg(theme.color(if settings.apps_colors() { "hi_fg" } else { "main_fg" }))),
    ];
    if !compact && settings.apps_cpu_graphs() {
      let history = sparkline(
        state.app_history().iter().map(|sample| {
          sample
            .get(app.name())
            .map(|stats| *stats.cpu_usage())
            .unwrap_or(f64::NAN)
        }),
        8,
        symbol,
        theme,
      );
      cells.push(
        Cell::from(history).style(Style::default().fg(theme.color(if settings.apps_colors() {
          "proc_misc"
        } else {
          "main_fg"
        }))),
      );
    }
    let cpu_color = if settings.apps_colors() && settings.apps_gradient() {
      theme.gradient("cpu", stats.map(|stats| *stats.cpu_usage() / 100.0).unwrap_or(0.0))
    } else {
      theme.color("main_fg")
    };
    cells.push(Cell::from(cpu).style(Style::default().fg(cpu_color)));
    let memory_color = if settings.apps_colors() && settings.apps_gradient() {
      let ratio = stats
        .zip(state.history().back())
        .map(|(stats, sample)| *stats.memory() as f64 / sample.memory().physical_memory_total() as f64)
        .unwrap_or(0.0);
      theme.gradient("used", ratio)
    } else {
      theme.color("main_fg")
    };
    cells.push(Cell::from(memory).style(Style::default().fg(memory_color)));
    if wide {
      cells.push(Cell::from(
        stats
          .map(|stats| format!("{}/s", bytes(stats.download())))
          .unwrap_or_else(|| "N/A".into()),
      ));
      cells.push(Cell::from(
        stats
          .map(|stats| format!("{}/s", bytes(stats.upload())))
          .unwrap_or_else(|| "N/A".into()),
      ));
    }
    Row::new(cells)
  });
  let mut columns = vec![
    Constraint::Min(7),
    Constraint::Length(if compact { 8 } else { 10 }),
    Constraint::Length(3),
  ];
  let mut headings = vec!["NAME", "STATE", "UPD"];
  if !compact && settings.apps_cpu_graphs() {
    columns.push(Constraint::Length(8));
    headings.push("CPU");
  }
  columns.extend([Constraint::Length(6), Constraint::Length(10)]);
  headings.extend([
    if settings.apps_per_core() { "CORE%" } else { "HOST%" },
    if settings.apps_mem_bytes() { "MEMORY" } else { "RAM%" },
  ]);
  if wide {
    columns.extend([Constraint::Length(13), Constraint::Length(13)]);
    headings.extend(["DOWNLOAD", "UPLOAD"]);
  }
  let mut table_state = TableState::default().with_selected(if apps.is_empty() { None } else { Some(ui.app_row) });
  frame.render_stateful_widget(
    Table::new(rows, columns)
      .header(Row::new(headings).style(Style::default().fg(theme.color("title")).add_modifier(Modifier::BOLD)))
      .row_highlight_style(
        Style::default()
          .fg(theme.color("selected_fg"))
          .bg(theme.color("selected_bg")),
      )
      .highlight_symbol("› "),
    table_area,
    &mut table_state,
  );
  super::super::mouse::rows(
    &mut ui.hits,
    Rect::new(
      table_area.x,
      table_area.y + 1,
      table_area.width,
      table_area.height.saturating_sub(1),
    ),
    table_state.offset(),
    apps.len(),
    false,
  );
  let detail_app = if let Some(name) = &ui.app_follow {
    all_apps.iter().find(|app| app.name() == name)
  } else {
    apps.get(ui.app_row).copied()
  };
  if let Some(app) = detail_app {
    let stats = stats.and_then(|stats| stats.get(app.name()));
    frame.render_widget(
      Paragraph::new(vec![
        Line::styled(
          format!(
            " {}  {}  update: {}",
            app.name(),
            app.human_version().as_deref().unwrap_or("version N/A"),
            app.update()
          ),
          Style::default().fg(theme.color("title")),
        ),
        Line::from(format!(
          " State: {}{}",
          app.state(),
          if ui.app_follow.is_some() { " | following" } else { "" }
        )),
        Line::from(
          stats
            .map(|stats| {
              let cpu = super::super::apps::cpu(*stats.cpu_usage(), state, settings)
                .map(|cpu| format!("{cpu:.1}%"))
                .unwrap_or_else(|| "N/A".into());
              format!(
                " CPU {cpu} {} | Memory {}",
                if settings.apps_per_core() { "core" } else { "host" },
                bytes(*stats.memory() as f64)
              )
            })
            .unwrap_or_else(|| " CPU N/A | Memory N/A".into()),
        ),
        Line::from(
          stats
            .map(|stats| format!(" Net ↓{}/s ↑{}/s", bytes(stats.download()), bytes(stats.upload())))
            .unwrap_or_else(|| " App metrics unavailable".into()),
        ),
        Line::from(
          stats
            .map(|stats| {
              format!(
                " Disk totals R {}  W {}",
                bytes(stats.blkio().read() as f64),
                bytes(stats.blkio().write() as f64)
              )
            })
            .unwrap_or_default(),
        ),
      ]),
      detail,
    );
  } else if let Some(name) = &ui.app_follow {
    frame.render_widget(Paragraph::new(format!("Following {name}: app unavailable")), detail);
  }
  let text = state
    .apps_error()
    .as_ref()
    .or(state.app_stats_error().as_ref())
    .map(|error| format!("stale: {error}"))
    .unwrap_or_else(|| {
      format!(
        "{} apps | stats {} | {} CPU%",
        apps.len(),
        age(*state.app_stats_updated_at()),
        if settings.apps_per_core() {
          "core; graphs host"
        } else {
          "host"
        }
      )
    });
  frame.render_widget(
    Paragraph::new(text).style(Style::default().fg(theme.color("graph_text"))),
    status,
  );
}

#[cfg(test)]
mod tests {
  use serde_json::json;

  #[test]
  fn it_keeps_followed_details_outside_filters_and_reports_disappearance() {
    let state = crate::client::fixtures::state();
    let mut ui = Ui::default();
    ui.app_details = true;
    ui.app_follow = Some("postgres".into());
    ui.filter = "forgejo".into();
    let settings = Settings::default();
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 25)).unwrap();
    terminal
      .draw(|frame| draw(frame, frame.area(), &state, &mut ui, &settings))
      .unwrap();
    let text: String = terminal
      .backend()
      .buffer()
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect();
    for expected in [
      "postgres  18.0",
      "State: RUNNING",
      "following",
      "CPU",
      "Memory",
      "Disk totals",
    ] {
      assert!(text.contains(expected));
    }
    terminal
      .draw(|frame| {
        draw(
          frame,
          frame.area(),
          &crate::client::fixtures::without_apps(),
          &mut ui,
          &settings,
        )
      })
      .unwrap();
    let text: String = terminal
      .backend()
      .buffer()
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect();
    assert!(text.contains("Following postgres: app unavailable"));
    ui.app_details = false;
    terminal
      .draw(|frame| draw(frame, frame.area(), &state, &mut ui, &settings))
      .unwrap();
    let text: String = terminal
      .backend()
      .buffer()
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect();
    assert!(!text.contains("Disk totals"));
  }

  use super::*;

  #[test]
  fn it_keeps_identity_through_sort_and_filter_and_renders_the_selected_units() {
    let state = crate::client::fixtures::state();
    let mut ui = Ui::default();
    ui.app_details = true;
    ui.app_selected = Some("postgres".into());
    let mut settings = Settings::default();
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 25)).unwrap();
    for sort in ["cpu", "memory", "name", "state"] {
      settings = settings.with_preference("apps_sort", json!(sort)).unwrap();
      for reversed in [false, true] {
        settings = settings.with_preference("apps_reversed", json!(reversed)).unwrap();
        terminal
          .draw(|frame| draw(frame, frame.area(), &state, &mut ui, &settings))
          .unwrap();
        assert_eq!(ui.app_selected.as_deref(), Some("postgres"));
        assert_eq!(
          super::super::super::apps::ordered(&state, "", &settings)[ui.app_row].name(),
          "postgres"
        );
      }
    }
    ui.filter = "forge".into();
    terminal
      .draw(|frame| draw(frame, frame.area(), &state, &mut ui, &settings))
      .unwrap();
    assert_eq!(ui.app_selected.as_deref(), Some("postgres"));
    ui.filter.clear();
    settings = settings
      .with_preference("apps_mem_bytes", json!(false))
      .unwrap()
      .with_preference("apps_per_core", json!(true))
      .unwrap()
      .with_preference("apps_cpu_graphs", json!(false))
      .unwrap()
      .with_preference("apps_colors", json!(false))
      .unwrap();
    terminal
      .draw(|frame| draw(frame, frame.area(), &state, &mut ui, &settings))
      .unwrap();
    let text: String = terminal
      .backend()
      .buffer()
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect();
    assert!(text.contains("postgres  18.0"));
    assert!(text.contains("CORE%"));
    assert!(text.contains("RAM%"));
    assert!(text.contains("2.3%"));
    assert!(!text.contains("GiB"));
    let missing = crate::client::fixtures::without_app_stats();
    terminal
      .draw(|frame| draw(frame, frame.area(), &missing, &mut ui, &settings))
      .unwrap();
    let text: String = terminal
      .backend()
      .buffer()
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect();
    assert!(text.contains("N/A"));
  }
}
