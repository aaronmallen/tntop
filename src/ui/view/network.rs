use super::*;
use crate::client::State;

pub(super) fn draw(frame: &mut Frame, area: Rect, state: &State, index: usize, theme: &Theme, settings: &Settings) {
  let symbol = graph_symbol(settings, theme, settings.graph_symbol_net());
  let bytes = |value| super::bytes(value, settings.base_10_sizes());
  let mut names: Vec<_> = state
    .history()
    .back()
    .map(|sample| sample.interfaces().keys().collect())
    .unwrap_or_default();
  if let Some(sample) = state.history().back() {
    names.sort_by_key(|name| {
      (
        sample.interfaces()[*name].link_state() != "LINK_STATE_UP",
        name.as_str() == "lo",
        name.as_str(),
      )
    });
  }
  let name = names.get(index % names.len().max(1)).copied();
  let block = panel(
    &format!(" net {} ", name.map(String::as_str).unwrap_or("")),
    "net_box",
    theme,
  );
  let inner = block.inner(area);
  frame.render_widget(block, area);
  let Some(name) = name else { return };
  let net = &state.history().back().unwrap().interfaces()[name];
  let down: Vec<_> = state
    .history()
    .iter()
    .map(|sample| {
      sample
        .interfaces()
        .get(name)
        .map(|net| net.received_bytes_rate())
        .unwrap_or(0.0)
    })
    .collect();
  let up: Vec<_> = state
    .history()
    .iter()
    .map(|sample| {
      sample
        .interfaces()
        .get(name)
        .map(|net| net.sent_bytes_rate())
        .unwrap_or(0.0)
    })
    .collect();
  let [chart, stats] =
    Layout::horizontal([Constraint::Min(1), Constraint::Length((inner.width / 2).clamp(16, 29))]).areas(inner);
  let [download, divider, upload] =
    Layout::vertical([Constraint::Min(0), Constraint::Length(1), Constraint::Min(0)]).areas(chart);
  let max_down = down.iter().copied().fold(1024.0, f64::max);
  let max_up = up.iter().copied().fold(1024.0, f64::max);
  frame.render_widget(
    Graph {
      values: &down,
      maximum: max_down,
      gradient: "download",
      theme,
      inverted: false,
      symbol,
    },
    download,
  );
  frame.render_widget(
    Graph {
      values: &up,
      maximum: max_up,
      gradient: "upload",
      theme,
      inverted: true,
      symbol,
    },
    upload,
  );
  frame.render_widget(
    Paragraph::new("▼ download / upload ▲")
      .alignment(Alignment::Center)
      .style(Style::default().fg(theme.color("graph_text"))),
    divider,
  );
  if download.height > 0 {
    frame.render_widget(
      Paragraph::new(format!("{}/s", bytes(max_down))).style(Style::default().fg(theme.color("graph_text"))),
      Rect::new(download.x, download.y, download.width, 1),
    );
  }
  if upload.height > 0 {
    frame.render_widget(
      Paragraph::new(format!("{}/s", bytes(max_up))).style(Style::default().fg(theme.color("graph_text"))),
      Rect::new(upload.x, upload.bottom() - 1, upload.width, 1),
    );
  }
  let totals = state.network_totals().get(name).cloned().unwrap_or_default();
  let speed = net
    .speed()
    .filter(|speed| *speed > 0.0)
    .map(|speed| format!("{speed:.0} Mb/s"))
    .unwrap_or_else(|| "speed N/A".into());
  let lines = vec![
    Line::styled(
      format!(
        " {} | {speed}",
        if net.link_state() == "LINK_STATE_UP" {
          "UP"
        } else {
          "DOWN"
        }
      ),
      Style::default().fg(theme.color("graph_text")),
    ),
    Line::styled(
      " download",
      Style::default().fg(theme.color("title")).add_modifier(Modifier::BOLD),
    ),
    Line::from(format!(" ▼ {}/s", bytes(net.received_bytes_rate()))),
    Line::from(format!(" Peak {}/s", bytes(totals.peak_download()))),
    Line::from(format!(" Session ~{}", bytes(totals.downloaded()))),
    Line::styled(
      " upload",
      Style::default().fg(theme.color("title")).add_modifier(Modifier::BOLD),
    ),
    Line::from(format!(" ▲ {}/s", bytes(net.sent_bytes_rate()))),
    Line::from(format!(" Peak {}/s", bytes(totals.peak_upload()))),
    Line::from(format!(" Session ~{}", bytes(totals.uploaded()))),
  ];
  if stats.height >= 9 {
    let centered = Rect::new(stats.x, stats.y + (stats.height - 9) / 2, stats.width, 9);
    frame.render_widget(Paragraph::new(lines), centered);
  } else {
    frame.render_widget(
      Paragraph::new(vec![
        lines[2].clone(),
        lines[6].clone(),
        Line::from(format!(
          " ~{} / {}",
          bytes(totals.downloaded()),
          bytes(totals.uploaded())
        )),
        lines[0].clone(),
      ]),
      stats,
    );
  }
}
