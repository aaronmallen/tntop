use super::*;
use crate::client::{Metrics, State};

pub(super) fn draw(
  frame: &mut Frame,
  area: Rect,
  state: &State,
  pool_index: usize,
  theme: &Theme,
  symbol: GraphSymbol,
) {
  let [memory, storage] = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(area);
  let block = panel(" mem / zfs ", "mem_box", theme);
  let inner = block.inner(memory);
  frame.render_widget(block, memory);
  let Some(metrics) = state.history().back() else {
    frame.render_widget(panel(" disks / pools ", "mem_box", theme), storage);
    return;
  };
  let total = metrics.memory().physical_memory_total() as f64;
  let [heading, charts, foot] =
    Layout::vertical([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)]).areas(inner);
  frame.render_widget(
    Paragraph::new(format!(" Total {}", bytes(total))).style(Style::default().fg(theme.color("title"))),
    heading,
  );
  let rows = Layout::vertical([Constraint::Ratio(1, 4); 4]).split(charts);
  for (index, (name, gradient)) in [
    ("Used", "used"),
    ("Available", "available"),
    ("ARC", "cached"),
    ("ZFS free", "free"),
  ]
  .into_iter()
  .enumerate()
  {
    let area = rows[index];
    if area.height == 0 {
      continue;
    }
    let history: Vec<_> = state
      .history()
      .iter()
      .map(|sample| value(sample, index).unwrap_or(0.0))
      .collect();
    let current = value(metrics, index);
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
  disks(frame, storage, state, pool_index, theme, symbol);
}

fn disks(frame: &mut Frame, area: Rect, state: &State, index: usize, theme: &Theme, symbol: GraphSymbol) {
  let block = panel(" disks / pools ", "mem_box", theme);
  let inner = block.inner(area);
  frame.render_widget(block, area);
  let Some(metrics) = state.history().back() else { return };
  let [pools, io] = Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(inner);
  let entries: Vec<_> = metrics.pools().iter().collect();
  let visible = (pools.height / 3).max(1) as usize;
  let start = index % entries.len().max(1);
  for (row, (name, pool)) in entries
    .iter()
    .cycle()
    .skip(start)
    .take(visible.min(entries.len()))
    .enumerate()
  {
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
  let Some(disks) = metrics.disks() else {
    frame.render_widget(Paragraph::new("Disk I/O unavailable"), io);
    return;
  };
  let [busy, chart, rates] =
    Layout::vertical([Constraint::Length(1), Constraint::Min(0), Constraint::Length(3)]).areas(io);
  meter(
    frame,
    busy,
    "I/O all",
    disks.busy() / 100.0,
    &format!("{:.0}%", disks.busy()),
    "used",
    theme,
  );
  let history: Vec<_> = state
    .history()
    .iter()
    .map(|sample| sample.disks().as_ref().map(|disks| disks.busy()).unwrap_or(0.0))
    .collect();
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
