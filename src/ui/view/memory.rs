use super::*;
use crate::client::{Metrics, State};

pub(super) fn draw(
  frame: &mut Frame,
  area: Rect,
  state: &State,
  selected: &mut Option<String>,
  theme: &Theme,
  settings: &Settings,
) {
  let symbol = graph_symbol(settings, theme, settings.graph_symbol_mem());
  let bytes = |value| super::bytes(value, settings.base_10_sizes());
  let [memory, storage] = if settings.show_disks() {
    Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(area)
  } else {
    [area, Rect::default()]
  };
  let block = panel("2 mem / zfs", "mem_box", theme);
  let inner = block.inner(memory);
  frame.render_widget(block, memory);
  let Some(metrics) = state.history().back() else {
    frame.render_widget(Paragraph::new("Memory unavailable"), inner);
    if settings.show_disks() {
      frame.render_widget(panel(" disks / pools ", "mem_box", theme), storage);
    }
    return;
  };
  let total = metrics.memory().physical_memory_total() as f64;
  let [heading, charts, foot] = Layout::vertical([
    Constraint::Length(1),
    Constraint::Min(0),
    Constraint::Length(u16::from(settings.show_arc())),
  ])
  .areas(inner);
  frame.render_widget(
    Paragraph::new(format!(" Total {}", bytes(total))).style(Style::default().fg(theme.color("title"))),
    heading,
  );
  let count = if settings.show_arc() { 4 } else { 2 };
  let rows = Layout::vertical(vec![Constraint::Ratio(1, count); count as usize]).split(charts);
  for (index, (name, gradient)) in [
    ("Used", "used"),
    ("Available", "available"),
    ("ARC", "cached"),
    ("ZFS free", "free"),
  ]
  .into_iter()
  .take(count as usize)
  .enumerate()
  {
    let area = rows[index];
    if area.height == 0 {
      continue;
    }
    let history: Vec<_> = state
      .history()
      .iter()
      .map(|sample| value(sample, index).unwrap_or(f64::NAN))
      .collect();
    let current = value(metrics, index);
    if !settings.mem_graphs() {
      let row = Rect::new(area.x, area.y, area.width, 1);
      if let Some(current) = current {
        meter(
          frame,
          row,
          name,
          current / total,
          &format!("{:.0}% {}", current / total * 100.0, bytes(current)),
          gradient,
          theme,
        );
      } else {
        frame.render_widget(Paragraph::new(format!("{name} N/A")), row);
      }
      continue;
    }
    let [label, chart] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
    let text = current
      .map(|value| format!("{name} {:.0}% {}", value / total * 100.0, bytes(value)))
      .unwrap_or_else(|| format!("{name} N/A"));
    frame.render_widget(
      Paragraph::new(text).style(Style::default().fg(theme.color("title"))),
      label,
    );
    if current.is_some() {
      frame.render_widget(
        Graph {
          values: &history,
          maximum: total,
          gradient,
          theme,
          inverted: false,
          symbol,
        },
        chart,
      );
    }
  }
  let hit = metrics
    .zfs()
    .as_ref()
    .and_then(|zfs| zfs.hit_ratio())
    .map(|ratio| format!("ARC hit {:.1}% | overlaps RAM", ratio * 100.0))
    .unwrap_or_else(|| "ARC overlaps RAM".into());
  frame.render_widget(
    Paragraph::new(hit).style(Style::default().fg(theme.color("graph_text"))),
    foot,
  );
  if settings.show_disks() {
    disks(frame, storage, state, selected, theme, settings);
  }
}

fn disks(
  frame: &mut Frame,
  area: Rect,
  state: &State,
  selected: &mut Option<String>,
  theme: &Theme,
  settings: &Settings,
) {
  let symbol = graph_symbol(settings, theme, settings.graph_symbol_mem());
  let bytes = |value| super::bytes(value, settings.base_10_sizes());
  let block = panel(" disks / pools ", "mem_box", theme);
  let inner = block.inner(area);
  frame.render_widget(block, area);
  let Some(metrics) = state.history().back() else { return };
  let [pools, io] = if !settings.show_io_stat() {
    [inner, Rect::default()]
  } else if settings.io_mode() {
    [Rect::default(), inner]
  } else {
    Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(inner)
  };
  let entries = super::super::storage::names(metrics, settings.disks_filter());
  if !selected.as_ref().is_some_and(|name| entries.contains(name)) {
    *selected = entries.first().cloned();
  }
  let visible = (pools.height / 3).max(1) as usize;
  let start = selected
    .as_ref()
    .and_then(|name| entries.iter().position(|entry| entry == name))
    .unwrap_or(0);
  if entries.is_empty() {
    frame.render_widget(Paragraph::new("No matching pools / unavailable"), pools);
  }
  for (row, name) in entries
    .iter()
    .cycle()
    .skip(start)
    .take(visible.min(entries.len()))
    .enumerate()
  {
    let pool = &metrics.pools()[name];
    let y = pools.y + row as u16 * 3;
    if y >= pools.bottom() {
      break;
    }
    frame.render_widget(
      Paragraph::new(format!("{name} {}", bytes(pool.total()))).style(Style::default().fg(theme.color("title"))),
      Rect::new(pools.x, y, pools.width, 1),
    );
    if y + 1 < pools.bottom() {
      meter(
        frame,
        Rect::new(pools.x, y + 1, pools.width, 1),
        "Used",
        pool.used() / pool.total().max(1.0),
        &format!("{:.0}%", pool.used() / pool.total().max(1.0) * 100.0),
        "used",
        theme,
      );
    }
    if y + 2 < pools.bottom() {
      frame.render_widget(
        Paragraph::new(format!("Free {}", bytes(pool.available())))
          .style(Style::default().fg(theme.color("graph_text"))),
        Rect::new(pools.x, y + 2, pools.width, 1),
      );
    }
  }
  if !settings.show_io_stat() {
    return;
  }
  let Some(disks) = metrics.disks() else {
    frame.render_widget(Paragraph::new("Disk I/O unavailable"), io);
    return;
  };
  let [busy, chart, rates] =
    Layout::vertical([Constraint::Length(1), Constraint::Min(0), Constraint::Length(3)]).areas(io);
  meter(
    frame,
    busy,
    "I/O all disks",
    disks.busy() / 100.0,
    &format!("{:.0}%", disks.busy()),
    "used",
    theme,
  );
  let history: Vec<_> = state
    .history()
    .iter()
    .map(|sample| sample.disks().as_ref().map(|disks| disks.busy()).unwrap_or(f64::NAN))
    .collect();
  if settings.io_mode() {
    let read: Vec<_> = state
      .history()
      .iter()
      .map(|sample| {
        sample
          .disks()
          .as_ref()
          .map(|disk| disk.read_bytes())
          .unwrap_or(f64::NAN)
      })
      .collect();
    let write: Vec<_> = state
      .history()
      .iter()
      .map(|sample| {
        sample
          .disks()
          .as_ref()
          .map(|disk| disk.write_bytes())
          .unwrap_or(f64::NAN)
      })
      .collect();
    let combined: Vec<_> = read.iter().zip(&write).map(|(read, write)| read + write).collect();
    let maximum = if settings.io_graph_scale() == 0 {
      if settings.io_graph_combined() {
        combined.iter().copied().fold(1.0, f64::max)
      } else {
        read.iter().chain(&write).copied().fold(1.0, f64::max)
      }
    } else {
      settings.io_graph_scale() as f64
    };
    let [label, graph] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(chart);
    frame.render_widget(
      Paragraph::new(format!(
        "All disks {} | {}/s",
        if settings.io_graph_combined() { "R+W" } else { "R / W" },
        bytes(maximum)
      )),
      label,
    );
    if settings.io_graph_combined() {
      frame.render_widget(
        Graph {
          values: &combined,
          maximum,
          gradient: "used",
          theme,
          inverted: false,
          symbol,
        },
        graph,
      );
    } else {
      let [read_area, write_area] =
        Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(graph);
      for (values, area, inverted) in [(&read, read_area, false), (&write, write_area, true)] {
        frame.render_widget(
          Graph {
            values,
            maximum,
            gradient: "used",
            theme,
            inverted,
            symbol,
          },
          area,
        );
      }
    }
  } else {
    frame.render_widget(
      Graph {
        values: &history,
        maximum: 100.0,
        gradient: "used",
        theme,
        inverted: false,
        symbol,
      },
      chart,
    );
  }
  frame.render_widget(
    Paragraph::new(vec![
      Line::from(format!("R {}/s", bytes(disks.read_bytes()))),
      Line::from(format!("W {}/s", bytes(disks.write_bytes()))),
      Line::from(format!("IOPS {:.0}r {:.0}w", disks.read_ops(), disks.write_ops())),
    ]),
    rates,
  );
}

fn value(metrics: &Metrics, index: usize) -> Option<f64> {
  let memory = metrics.memory();
  match index {
    0 => Some((memory.physical_memory_total() - memory.physical_memory_available()) as f64),
    1 => Some(memory.physical_memory_available() as f64),
    2 => Some(memory.arc_size() as f64),
    _ => memory.arc_free_memory().map(|value| value as f64),
  }
}

#[cfg(test)]
mod tests {
  use serde_json::json;

  use super::*;

  mod draw {
    use super::*;

    #[test]
    fn it_labels_aggregate_io_modes_and_scales_without_attributing_them_to_pools() {
      let state = crate::client::fixtures::state();
      let theme = Theme::default();
      let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(160, 30)).unwrap();
      for combined in [false, true] {
        let settings = Settings::default()
          .with_preference("io_mode", json!(true))
          .unwrap()
          .with_preference("io_graph_combined", json!(combined))
          .unwrap()
          .with_preference("io_graph_scale", json!(1048576))
          .unwrap();
        draw_frame(&mut terminal, &state, &theme, &settings);
        let output = text(&terminal);
        assert!(output.contains("I/O all disks"));
        assert!(output.contains(if combined { "All disks R+W" } else { "All disks R / W" }));
        assert!(output.contains("1.0 MiB/s"));
        assert!(!output.contains("storage_share"));
        let settings = settings.with_preference("show_io_stat", json!(false)).unwrap();
        draw_frame(&mut terminal, &state, &theme, &settings);
        let output = text(&terminal);
        assert!(output.contains("storage_share"));
        assert!(!output.contains("I/O all disks"));
      }
      let settings = Settings::default()
        .with_preference("disks_filter", json!("missing"))
        .unwrap();
      draw_frame(&mut terminal, &state, &theme, &settings);
      assert!(text(&terminal).contains("No matching pools"));
      assert!(text(&terminal).contains("I/O all disks"));
      let metrics = serde_json::from_value(json!({
        "cpu":{"cpu":{"usage":0}}, "interfaces":{},
        "memory":{"physical_memory_total":1000,"physical_memory_available":400,"arc_size":500}
      }))
      .unwrap();
      draw_frame(
        &mut terminal,
        &crate::client::fixtures::with_metrics(metrics),
        &theme,
        &settings,
      );
      assert!(text(&terminal).contains("Disk I/O unavailable"));
    }

    #[test]
    fn it_keeps_memory_meaning_across_graphs_meters_and_storage_visibility() {
      let state = crate::client::fixtures::state();
      let theme = Theme::default();
      let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 24)).unwrap();
      for graphs in [true, false] {
        for storage in [true, false] {
          let settings = Settings::default()
            .with_preference("mem_graphs", json!(graphs))
            .unwrap()
            .with_preference("show_disks", json!(storage))
            .unwrap();
          draw_frame(&mut terminal, &state, &theme, &settings);
          let output = text(&terminal);
          for expected in ["12.0 GiB", "4.0 GiB", "8.0 GiB", "1.0 GiB", "overlaps RAM"] {
            assert!(output.contains(expected), "{expected} missing");
          }
          assert_eq!(output.contains("disks / pools"), storage);
          if !storage {
            assert_eq!(terminal.backend().buffer()[(60, 0)].symbol(), "─");
          }
          let settings = settings.with_preference("show_arc", json!(false)).unwrap();
          draw_frame(&mut terminal, &state, &theme, &settings);
          let text = text(&terminal);
          assert!(text.contains("12.0 GiB"));
          assert!(text.contains("4.0 GiB"));
          assert!(!text.contains("ARC"));
          assert!(!text.contains("ZFS free"));
        }
      }
      let metrics = serde_json::from_value(json!({
        "cpu":{"cpu":{"usage":0}}, "interfaces":{},
        "memory":{"physical_memory_total":1000,"physical_memory_available":400,"arc_size":500}
      }))
      .unwrap();
      let state = crate::client::fixtures::with_metrics(metrics);
      for graphs in [true, false] {
        let settings = Settings::default()
          .with_preference("mem_graphs", json!(graphs))
          .unwrap();
        draw_frame(&mut terminal, &state, &theme, &settings);
        assert!(text(&terminal).contains("ZFS free N/A"));
        assert_eq!(value(state.history().back().unwrap(), 0), Some(600.0));
        assert_eq!(value(state.history().back().unwrap(), 2), Some(500.0));
        assert_eq!(value(state.history().back().unwrap(), 3), None);
      }
    }

    fn draw_frame(
      terminal: &mut ratatui::Terminal<ratatui::backend::TestBackend>,
      state: &State,
      theme: &Theme,
      settings: &Settings,
    ) {
      terminal
        .draw(|frame| draw(frame, frame.area(), state, &mut None, theme, settings))
        .unwrap();
    }

    fn text(terminal: &ratatui::Terminal<ratatui::backend::TestBackend>) -> String {
      terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect()
    }
  }
}
