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
  let mut cores: Vec<_> = metrics
    .cpu()
    .iter()
    .filter(|(name, _)| name.as_str() != "cpu")
    .collect();
  cores.sort_by_key(|(name, _)| name.trim_start_matches("cpu").parse::<usize>().unwrap_or(usize::MAX));
  let digits = cores
    .iter()
    .map(|(name, _)| name.trim_start_matches("cpu").len())
    .max()
    .unwrap_or(1)
    .max(3);
  let temperatures = settings.check_temp() && settings.show_coretemp();
  let fixed_width = digits as u16 + 9 + if temperatures { 6 } else { 0 };
  let budget = inner.width - inner.width / 3;
  let needed = (cores.len() as u16)
    .div_ceil(inner.height.saturating_sub(4).max(1))
    .max(1);
  let columns = needed.min((budget / fixed_width).max(1));
  let column_width = (budget / columns).min(30.max(fixed_width));
  let sidebar = (column_width * columns).min(inner.width.saturating_sub(1));
  let [chart, details] = Layout::horizontal([Constraint::Min(1), Constraint::Length(sidebar)]).areas(inner);
  let [top, divider, bottom, uptime] = Layout::vertical([
    Constraint::Min(1),
    Constraint::Length(1),
    if settings.cpu_single_graph() {
      Constraint::Length(0)
    } else {
      Constraint::Min(1)
    },
    Constraint::Length(u16::from(settings.show_uptime())),
  ])
  .areas(chart);
  let upper = if settings.cpu_graph_upper() == "auto" {
    "total"
  } else {
    settings.cpu_graph_upper()
  };
  let lower = if settings.cpu_graph_lower() == "auto" {
    "busiest"
  } else {
    settings.cpu_graph_lower()
  };
  let mut labels = Vec::new();
  for (source, area, inverted) in [(upper, top, false), (lower, bottom, settings.cpu_invert_lower())]
    .into_iter()
    .take(if settings.cpu_single_graph() { 1 } else { 2 })
  {
    let values: Vec<_> = state
      .history()
      .iter()
      .map(|sample| usage(sample, source).unwrap_or(f64::NAN))
      .collect();
    let label = if source == "busiest" { "busiest core" } else { source };
    labels.push(format!(
      "{label} {}",
      if usage(metrics, source).is_none() {
        "N/A"
      } else if inverted {
        "▼"
      } else {
        "▲"
      }
    ));
    if usage(metrics, source).is_none() {
      frame.render_widget(Paragraph::new(format!("{label}: N/A")).centered(), area);
    } else {
      frame.render_widget(
        Graph {
          values: &values,
          maximum: 100.0,
          gradient: "cpu",
          theme,
          inverted,
          symbol,
        },
        area,
      );
    }
  }
  frame.render_widget(
    Paragraph::new(labels.join(" / "))
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
    Paragraph::new(if settings.custom_cpu_name().is_empty() {
      state
        .system_info()
        .as_ref()
        .map(|info| info.model().as_str())
        .unwrap_or("CPU model unavailable")
    } else {
      settings.custom_cpu_name()
    })
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
        if settings.check_temp() {
          temperature(cpu.temp(), settings.temp_scale())
        } else {
          String::new()
        }
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
      width.saturating_sub(fixed_width),
      symbol,
      theme,
    );
    frame.render_widget(
      Paragraph::new(Line::from(vec![
        Span::raw(format!(" C{:<digits$}", name.trim_start_matches("cpu"))),
        Span::styled(history, Style::default().fg(theme.gradient("cpu", cpu.usage() / 100.0))),
        Span::raw(format!(" {:3.0}% ", cpu.usage())),
        Span::styled(
          if temperatures {
            temperature(cpu.temp(), settings.temp_scale())
          } else {
            String::new()
          },
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

fn temperature(celsius: Option<f64>, scale: &str) -> String {
  let Some(celsius) = celsius else {
    return "N/A".into();
  };
  let (value, unit) = match scale {
    "fahrenheit" => (celsius * 1.8 + 32.0, "°F"),
    "kelvin" => (celsius + 273.15, "K"),
    "rankine" => ((celsius + 273.15) * 1.8, "°R"),
    _ => (celsius, "°C"),
  };
  format!("{value:.0}{unit}")
}

fn usage(metrics: &crate::client::Metrics, source: &str) -> Option<f64> {
  match source {
    "busiest" => metrics
      .cpu()
      .iter()
      .filter(|(name, _)| name.as_str() != "cpu")
      .map(|(_, cpu)| cpu.usage())
      .reduce(f64::max),
    "total" => metrics.cpu().get("cpu").map(|cpu| cpu.usage()),
    name => metrics.cpu().get(name).map(|cpu| cpu.usage()),
  }
}

#[cfg(test)]
mod tests {
  use serde_json::json;

  use super::*;

  mod draw {
    use super::*;

    #[test]
    fn it_fits_cores_by_height_and_width_and_reflows_after_resizing() {
      for count in [32, 128] {
        let mut cpus = serde_json::Map::new();
        cpus.insert("cpu".into(), json!({"usage":25, "temp":50}));
        for index in 0..count {
          cpus.insert(format!("cpu{index}"), json!({"usage":25, "temp":50}));
        }
        let state = crate::client::fixtures::with_metrics(
          serde_json::from_value(json!({
            "cpu":cpus, "interfaces":{},
            "memory":{"physical_memory_total":1000,"physical_memory_available":500,"arc_size":100}
          }))
          .unwrap(),
        );
        for temps in [true, false] {
          let settings = Settings::default()
            .with_preference("show_coretemp", json!(temps))
            .unwrap();
          for (width, height, fits) in [(160, 20, count == 32), (240, 40, true), (40, 10, false), (1, 1, false)] {
            let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
            terminal
              .draw(|frame| draw(frame, frame.area(), &state, &Theme::default(), &settings))
              .unwrap();
            let text: String = terminal
              .backend()
              .buffer()
              .content
              .iter()
              .map(|cell| cell.symbol())
              .collect();
            if fits {
              for index in 0..count {
                assert!(
                  text.contains(&format!(" C{index:<3}")),
                  "missing core {index} at {width}x{height}"
                );
              }
              assert!(!text.contains("enlarge terminal"));
            } else if width >= 40 {
              assert!(!text.contains(&format!(" C{:<3}", count - 1)));
            }
          }
        }
      }
    }

    #[test]
    fn it_handles_changed_core_counts_units_and_hidden_readings() {
      let theme = Theme::default();
      let mut settings = Settings::default()
        .with_preference("cpu_graph_upper", json!("cpu3"))
        .unwrap()
        .with_preference("custom_cpu_name", json!("My CPU"))
        .unwrap()
        .with_preference("temp_scale", json!("fahrenheit"))
        .unwrap();
      let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 20)).unwrap();
      let state = crate::client::fixtures::state();
      terminal
        .draw(|frame| draw(frame, frame.area(), &state, &theme, &settings))
        .unwrap();
      let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
      assert!(text.contains("My CPU"));
      assert!(text.contains("144°F"));
      assert!(!text.contains("cpu3: N/A"));
      let metrics = serde_json::from_value(json!({
        "cpu": {"cpu":{"usage":25}, "cpu0":{"usage":50}},
        "interfaces": {},
        "memory": {"physical_memory_total":1000,"physical_memory_available":500,"arc_size":100}
      }))
      .unwrap();
      let state = crate::client::fixtures::with_metrics(metrics);
      terminal
        .draw(|frame| draw(frame, frame.area(), &state, &theme, &settings))
        .unwrap();
      let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
      assert!(text.contains("cpu3: N/A"));
      assert!(text.contains("25% N/A"));
      settings = settings
        .with_preference("cpu_graph_upper", json!("total"))
        .unwrap()
        .with_preference("cpu_single_graph", json!(true))
        .unwrap()
        .with_preference("check_temp", json!(false))
        .unwrap()
        .with_preference("show_uptime", json!(false))
        .unwrap();
      terminal
        .draw(|frame| draw(frame, frame.area(), &state, &theme, &settings))
        .unwrap();
      let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
      assert!(!text.contains("busiest"));
      assert!(!text.contains("N/A"));
      assert!(!text.contains(" up "));
    }
  }

  mod temperature {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_converts_existing_readings_and_preserves_missing_values() {
      for (scale, expected) in [
        ("celsius", "0°C"),
        ("fahrenheit", "32°F"),
        ("kelvin", "273K"),
        ("rankine", "492°R"),
      ] {
        assert_eq!(temperature(Some(0.0), scale), expected);
        assert_eq!(temperature(None, scale), "N/A");
      }
    }
  }
}
