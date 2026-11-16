use super::*;
use crate::client::State;

pub(super) fn draw(frame: &mut Frame, area: Rect, state: &State, theme: &Theme, settings: &Settings) {
  let symbol = graph_symbol(settings, theme, settings.graph_symbol_cpu());
  let block = panel(" cpu ", "cpu_box", theme);
  let inner = block.inner(area);
  frame.render_widget(block, area);
  let Some(metrics) = state.history().back() else {
    frame.render_widget(
      Paragraph::new(state.error().as_deref().unwrap_or("Connecting to TrueNAS…")).wrap(Wrap {
        trim: true,
      }),
      inner,
    );
    return;
  };
  let sidebar = (inner.width / 3).clamp(28, 64);
  let [chart, details] = Layout::horizontal([Constraint::Min(1), Constraint::Length(sidebar)]).areas(inner);
  let [top, divider, bottom, uptime] = Layout::vertical([
    Constraint::Min(1),
    Constraint::Length(1),
    Constraint::Min(1),
    Constraint::Length(1),
  ])
  .areas(chart);
  let total: Vec<_> = state
    .history()
    .iter()
    .map(|sample| sample.cpu()["cpu"].usage())
    .collect();
  let busiest: Vec<_> = state
    .history()
    .iter()
    .map(|sample| {
      sample
        .cpu()
        .iter()
        .filter(|(key, _)| key.as_str() != "cpu")
        .map(|(_, cpu)| cpu.usage())
        .fold(0.0, f64::max)
    })
    .collect();
  frame.render_widget(
    Graph {
      values: &total,
      maximum: 100.0,
      gradient: "cpu",
      theme,
      inverted: false,
      symbol,
    },
    top,
  );
  frame.render_widget(
    Graph {
      values: &busiest,
      maximum: 100.0,
      gradient: "cpu",
      theme,
      inverted: true,
      symbol,
    },
    bottom,
  );
  frame.render_widget(
    Paragraph::new("total ▲ / busiest core ▼")
      .alignment(Alignment::Center)
      .style(Style::default().fg(theme.color("graph_text"))),
    divider,
  );
  let up = state
    .system_info()
    .as_ref()
    .map(|info| {
      let seconds = *info.uptime_seconds()
        + state
          .system_updated_at()
          .map(|time| time.elapsed().as_secs_f64())
          .unwrap_or(0.0);
      let seconds = seconds.max(0.0) as u64;
      format!(
        " up {}d {:02}:{:02}:{:02}",
        seconds / 86400,
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60
      )
    })
    .unwrap_or_else(|| " uptime unavailable".into());
  frame.render_widget(
    Paragraph::new(up).style(Style::default().fg(theme.color("graph_text"))),
    uptime,
  );
  let mut cores: Vec<_> = metrics
    .cpu()
    .iter()
    .filter(|(name, _)| name.as_str() != "cpu")
    .collect();
  cores.sort_by_key(|(name, _)| name.trim_start_matches("cpu").parse::<usize>().unwrap_or(usize::MAX));
  let columns = (details.width / 30).max(1);
  let rows = (cores.len() as u16).div_ceil(columns);
  let height = (rows + 4).min(details.height);
  let details = Rect::new(
    details.x,
    details.y + (details.height - height) / 2,
    details.width,
    height,
  );
  if height == 0 {
    return;
  }
  frame.render_widget(
    Paragraph::new(
      state
        .system_info()
        .as_ref()
        .map(|info| info.model().as_str())
        .unwrap_or("CPU model unavailable"),
    )
    .alignment(Alignment::Center)
    .style(Style::default().fg(theme.color("title"))),
    Rect::new(details.x, details.y, details.width, 1),
  );
  if height > 1 {
    let cpu = &metrics.cpu()["cpu"];
    meter(
      frame,
      Rect::new(details.x, details.y + 1, details.width, 1),
      "CPU",
      cpu.usage() / 100.0,
      &format!(
        "{:.0}% {}",
        cpu.usage(),
        cpu.temp().map(|temp| format!("{temp:.0}°C")).unwrap_or_default()
      ),
      "cpu",
      theme,
    );
  }
  let capacity = height.saturating_sub(4) as usize * columns as usize;
  for (index, (name, cpu)) in cores.iter().take(capacity).enumerate() {
    let row = index / columns as usize;
    let col = index % columns as usize;
    let width = details.width / columns;
    let area = Rect::new(details.x + col as u16 * width, details.y + 2 + row as u16, width, 1);
    let history = sparkline(
      state
        .history()
        .iter()
        .map(|sample| sample.cpu().get(*name).map(|cpu| cpu.usage()).unwrap_or(0.0)),
      width.saturating_sub(18),
      symbol,
      theme,
    );
    frame.render_widget(
      Paragraph::new(Line::from(vec![
        Span::raw(format!(" C{:<3}", name.trim_start_matches("cpu"))),
        Span::styled(history, Style::default().fg(theme.gradient("cpu", cpu.usage() / 100.0))),
        Span::raw(format!(" {:3.0}% ", cpu.usage())),
        Span::styled(
          cpu
            .temp()
            .map(|temp| format!("{temp:2.0}°C"))
            .unwrap_or_else(|| " N/A".into()),
          Style::default().fg(theme.gradient("temp", cpu.temp().unwrap_or(0.0) / 100.0)),
        ),
      ])),
      area,
    );
  }
  if height >= 3 {
    let load = state
      .system_info()
      .as_ref()
      .map(|info| {
        format!(
          " Load avg: {:.2} {:.2} {:.2}",
          info.loadavg()[0],
          info.loadavg()[1],
          info.loadavg()[2]
        )
      })
      .unwrap_or_else(|| " Load avg: unavailable".into());
    frame.render_widget(
      Paragraph::new(load),
      Rect::new(details.x, details.bottom() - 2, details.width, 1),
    );
    let status = if cores.len() > capacity {
      format!(" +{} cores | enlarge terminal", cores.len() - capacity)
    } else {
      state
        .system_error()
        .as_deref()
        .map(|error| format!(" Info: {error}"))
        .unwrap_or_else(|| format!(" {} cores | info {}", cores.len(), age(*state.system_updated_at())))
    };
    frame.render_widget(
      Paragraph::new(status).style(Style::default().fg(theme.color("graph_text"))),
      Rect::new(details.x, details.bottom() - 1, details.width, 1),
    );
  }
}
