use super::*;
use crate::client::State;

pub(super) fn draw(frame: &mut Frame, area: Rect, state: &State, ui: &mut Ui, settings: &Settings) {
  let theme = &ui.theme;
  let symbol = graph_symbol(settings, theme, settings.graph_symbol_net());
  let bytes = |value| super::bytes(value, settings.base_10_sizes());
  let rate = |value| rate(value, settings);
  let name = super::super::network::selected(state, settings);
  let block = panel(&format!(" 3 net {} ", name.unwrap_or("")), "net_box", theme);
  let inner = block.inner(area);
  frame.render_widget(block, area);
  let controls = Rect::new(
    inner.x,
    area.bottom().saturating_sub(1),
    inner.width,
    u16::from(area.height > 1),
  );
  super::super::mouse::controls(
    frame,
    &mut ui.hits,
    controls,
    theme,
    &[
      ("", "b", " ‹", KeyCode::Char('b')),
      ("", "n", " ›", KeyCode::Char('n')),
      (
        "",
        "a",
        if settings.net_auto() { "uto" } else { "uto off" },
        KeyCode::Char('a'),
      ),
      (
        "s",
        "y",
        if settings.net_sync() { "nc" } else { "nc off" },
        KeyCode::Char('y'),
      ),
      ("", "z", "ero", KeyCode::Char('z')),
    ],
  );
  let Some(name) = name else {
    frame.render_widget(Paragraph::new("Network unavailable"), inner);
    return;
  };
  let net = &state.history().back().unwrap().interfaces()[name];
  let down: Vec<_> = state
    .history()
    .iter()
    .map(|sample| {
      sample
        .interfaces()
        .get(name)
        .map(|net| net.received_bytes_rate())
        .unwrap_or(f64::NAN)
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
        .unwrap_or(f64::NAN)
    })
    .collect();
  let [chart, stats] =
    Layout::horizontal([Constraint::Min(1), Constraint::Length((inner.width / 2).clamp(16, 29))]).areas(inner);
  let [download, divider, upload] =
    Layout::vertical([Constraint::Min(0), Constraint::Length(1), Constraint::Min(0)]).areas(chart);
  let (download, upload) = if settings.net_swap() {
    (upload, download)
  } else {
    (download, upload)
  };
  let (max_down, max_up) = super::super::network::scales(&down, &up, settings);
  frame.render_widget(
    Graph {
      values: &down,
      maximum: max_down,
      gradient: "download",
      theme,
      inverted: settings.net_swap(),
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
      inverted: !settings.net_swap(),
      symbol,
    },
    upload,
  );
  frame.render_widget(
    Paragraph::new(if settings.net_swap() {
      "▲ upload / download ▼"
    } else {
      "▼ download / upload ▲"
    })
    .alignment(Alignment::Center)
    .style(Style::default().fg(theme.color("graph_text"))),
    divider,
  );
  if download.height > 0 {
    frame.render_widget(
      Paragraph::new(format!("↓ {}", rate(max_down))).style(Style::default().fg(theme.color("graph_text"))),
      Rect::new(download.x, download.y, download.width, 1),
    );
  }
  if upload.height > 0 {
    frame.render_widget(
      Paragraph::new(format!("↑ {}", rate(max_up))).style(Style::default().fg(theme.color("graph_text"))),
      Rect::new(upload.x, upload.bottom() - 1, upload.width, 1),
    );
  }
  let totals = state.network_totals().get(name).cloned().unwrap_or_default();
  let baseline = ui.net_baselines.get(&(ui.selected, name.to_owned()));
  let (base_down, base_up) = baseline.copied().unwrap_or_default();
  let downloaded = (totals.downloaded() - base_down).max(0.0);
  let uploaded = (totals.uploaded() - base_up).max(0.0);
  let total_label = if baseline.is_some() { "Reset ~" } else { "Session ~" };
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
    Line::from(format!(" ▼ {}", rate(net.received_bytes_rate()))),
    Line::from(format!(" Peak {}", rate(totals.peak_download()))),
    Line::from(format!(" {total_label}{}", bytes(downloaded))),
    Line::styled(
      " upload",
      Style::default().fg(theme.color("title")).add_modifier(Modifier::BOLD),
    ),
    Line::from(format!(" ▲ {}", rate(net.sent_bytes_rate()))),
    Line::from(format!(" Peak {}", rate(totals.peak_upload()))),
    Line::from(format!(" {total_label}{}", bytes(uploaded))),
  ];
  if stats.height >= 9 {
    let centered = Rect::new(stats.x, stats.y + (stats.height - 9) / 2, stats.width, 9);
    frame.render_widget(Paragraph::new(lines), centered);
  } else {
    frame.render_widget(
      Paragraph::new(vec![
        lines[2].clone(),
        lines[6].clone(),
        Line::from(format!(" {total_label}{} / {}", bytes(downloaded), bytes(uploaded))),
        lines[0].clone(),
      ]),
      stats,
    );
  }
}

fn rate(value: f64, settings: &Settings) -> String {
  if settings.net_bitrate() {
    format!("{}/s", bytes(value * 8.0, settings.base_10_sizes()).replace('B', "bit"))
  } else {
    format!("{}/s", bytes(value, settings.base_10_sizes()))
  }
}

#[cfg(test)]
mod tests {
  use serde_json::json;

  use super::*;

  #[test]
  fn it_keeps_direction_labels_and_estimates_when_swapping_graphs() {
    let state = crate::client::fixtures::state();
    let mut ui = Ui::default();
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 30)).unwrap();
    for swap in [false, true] {
      let settings = Settings::default().with_preference("net_swap", json!(swap)).unwrap();
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
      assert!(text.contains(if swap { "upload / download" } else { "download / upload" }));
      assert!(text.contains("Session ~"));
      assert!(text.contains("auto"));
      assert!(text.contains("sync off"));
    }
    let totals = &state.network_totals()["enp1s0"];
    ui.net_baselines
      .insert((0, "enp1s0".into()), (totals.downloaded(), totals.uploaded()));
    terminal
      .draw(|frame| draw(frame, frame.area(), &state, &mut ui, &Settings::default()))
      .unwrap();
    let text: String = terminal
      .backend()
      .buffer()
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect();
    assert!(text.contains("Reset ~0.0 B"));
  }

  #[test]
  fn it_formats_bits_in_the_selected_base_without_changing_byte_totals() {
    let mut settings = Settings::default().with_preference("net_bitrate", json!(true)).unwrap();
    assert_eq!(rate(128.0, &settings), "1.0 Kibit/s");
    settings = settings.with_preference("base_10_sizes", json!(true)).unwrap();
    assert_eq!(rate(125.0, &settings), "1.0 kbit/s");
    settings = settings.with_preference("net_bitrate", json!(false)).unwrap();
    assert_eq!(rate(1000.0, &settings), "1.0 kB/s");
  }
}
