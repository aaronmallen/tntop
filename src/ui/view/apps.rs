use ratatui::widgets::Cell;

use super::*;
use crate::client::State;

pub(super) fn draw(frame: &mut Frame, area: Rect, state: &State, ui: &mut Ui, symbol: GraphSymbol) {
  let theme = &ui.theme;
  let sort = ["cpu", "memory", "name", "state"][ui.sort];
  let title = format!(" apps / {sort} {}{} ", ui.filter, if ui.filtering { "▏" } else { "" });
  let block = panel(&title, "proc_box", theme);
  let inner = block.inner(area);
  frame.render_widget(block, area);
  let Some(apps) = state.apps() else {
    frame.render_widget(
      Paragraph::new(state.apps_error().as_deref().unwrap_or("Waiting for installed apps…")).wrap(Wrap {
        trim: true,
      }),
      inner,
    );
    return;
  };
  let [table_area, detail, status] =
    Layout::vertical([Constraint::Min(1), Constraint::Length(3), Constraint::Length(1)]).areas(inner);
  let stats = state.app_history().back();
  let mut apps: Vec<_> = apps
    .iter()
    .filter(|app| app.name().to_lowercase().contains(&ui.filter.to_lowercase()))
    .collect();
  apps.sort_by(|a, b| {
    let a_stats = stats.and_then(|stats| stats.get(a.name()));
    let b_stats = stats.and_then(|stats| stats.get(b.name()));
    let order = match ui.sort {
      0 => b_stats
        .map(|stats| *stats.cpu_usage())
        .partial_cmp(&a_stats.map(|stats| *stats.cpu_usage()))
        .unwrap_or(std::cmp::Ordering::Equal),
      1 => b_stats
        .map(|stats| *stats.memory())
        .cmp(&a_stats.map(|stats| *stats.memory())),
      3 => a.state().cmp(b.state()),
      _ => a.name().cmp(b.name()),
    };
    order.then_with(|| a.name().cmp(b.name()))
  });
  ui.app_row = ui.app_row.min(apps.len().saturating_sub(1));
  let wide = inner.width >= 85;
  let compact = inner.width < 55;
  let rows = apps.iter().map(|app| {
    let stats = stats.and_then(|stats| stats.get(app.name()));
    let color = match app.state().as_str() {
      "RUNNING" => theme.gradient("cpu", 0.0),
      "CRASHED" => theme.gradient("cpu", 1.0),
      "STOPPED" => theme.color("inactive_fg"),
      _ => theme.gradient("cpu", 0.5),
    };
    let cpu = stats
      .map(|stats| format!("{:5.1}", stats.cpu_usage()))
      .unwrap_or_else(|| "  N/A".into());
    let memory = stats
      .map(|stats| bytes(*stats.memory() as f64))
      .unwrap_or_else(|| "N/A".into());
    let mut cells = vec![
      Cell::from(app.name().as_str()),
      Cell::from(app.state().as_str()).style(Style::default().fg(color)),
      Cell::from(if app.update() == "available" { "YES" } else { "-" })
        .style(Style::default().fg(theme.color("hi_fg"))),
    ];
    if !compact {
      let history = sparkline(
        state
          .app_history()
          .iter()
          .map(|sample| sample.get(app.name()).map(|stats| *stats.cpu_usage()).unwrap_or(0.0)),
        8,
        symbol,
        theme,
      );
      cells.push(Cell::from(history).style(Style::default().fg(theme.color("proc_misc"))));
    }
    cells.push(Cell::from(cpu));
    cells.push(Cell::from(memory));
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
  if !compact {
    columns.push(Constraint::Length(8));
    headings.push("CPU");
  }
  columns.extend([Constraint::Length(5), Constraint::Length(10)]);
  headings.extend(["CPU%", "MEMORY"]);
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
  if let Some(app) = apps.get(ui.app_row) {
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
  }
  let text = state
    .apps_error()
    .as_ref()
    .or(state.app_stats_error().as_ref())
    .map(|error| format!("stale: {error}"))
    .unwrap_or_else(|| {
      format!(
        "{} apps | stats {} | host CPU%",
        apps.len(),
        age(*state.app_stats_updated_at())
      )
    });
  frame.render_widget(
    Paragraph::new(text).style(Style::default().fg(theme.color("graph_text"))),
    status,
  );
}
