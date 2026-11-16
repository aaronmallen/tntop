use std::{collections::HashMap, path::Path};

use ratatui::style::{Color, Style};

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("invalid btop theme at line {0}")]
  Invalid(usize),
  #[error("cannot read theme: {0}")]
  Io(#[from] std::io::Error),
}

pub struct Theme(HashMap<String, Color>);

impl Theme {
  pub fn load(path: Option<&Path>) -> Result<Self, Error> {
    match path {
      Some(path) => Self::parse(&std::fs::read_to_string(path)?),
      None => Ok(Self::default()),
    }
  }

  fn parse(source: &str) -> Result<Self, Error> {
    let mut theme = Self::default();
    let mut supplied = HashMap::new();
    for (index, line) in source.lines().enumerate() {
      let line = line.trim().trim_start_matches('\u{feff}');
      if line.is_empty() || line.starts_with('#') {
        continue;
      }
      let (key, value) = line
        .strip_prefix("theme[")
        .and_then(|line| line.split_once(']'))
        .ok_or(Error::Invalid(index + 1))?;
      if !theme.0.contains_key(key) {
        continue;
      }
      let value = value.trim().strip_prefix('=').ok_or(Error::Invalid(index + 1))?.trim();
      let value = if let Some(value) = value.strip_prefix('"') {
        value.split_once('"').ok_or(Error::Invalid(index + 1))?.0
      } else {
        value
      };
      if value.is_empty() && key != "main_bg" && !key.ends_with("_mid") && !key.ends_with("_end") {
        continue;
      }
      let color = parse_color(value).ok_or(Error::Invalid(index + 1))?;
      supplied.insert(key.to_owned(), color);
      theme.0.insert(key.to_owned(), color);
    }
    if !supplied.contains_key("meter_bg") {
      theme.0.insert("meter_bg".into(), theme.color("inactive_fg"));
    }
    if !supplied.contains_key("graph_text") {
      theme.0.insert("graph_text".into(), theme.color("inactive_fg"));
    }
    if !supplied.contains_key("process_start") {
      for suffix in ["start", "mid", "end"] {
        theme
          .0
          .insert(format!("process_{suffix}"), theme.color(&format!("cpu_{suffix}")));
      }
    }
    Ok(theme)
  }

  pub fn color(&self, key: &str) -> Color {
    self.0.get(key).copied().unwrap_or(Color::Reset)
  }

  pub fn gradient(&self, name: &str, value: f64) -> Color {
    let start = self.color(&format!("{name}_start"));
    let mid = self.color(&format!("{name}_mid"));
    let end = self.color(&format!("{name}_end"));
    let value = value.clamp(0.0, 1.0);
    if end == Color::Reset {
      return start;
    }
    let (a, b, ratio) = match mid {
      Color::Reset => (start, end, value),
      _ if value < 0.5 => (start, mid, value * 2.0),
      _ => (mid, end, value * 2.0 - 1.0),
    };
    match (a, b) {
      (Color::Rgb(ar, ag, ab), Color::Rgb(br, bg, bb)) => Color::Rgb(
        (ar as f64 + (br as f64 - ar as f64) * ratio) as u8,
        (ag as f64 + (bg as f64 - ag as f64) * ratio) as u8,
        (ab as f64 + (bb as f64 - ab as f64) * ratio) as u8,
      ),
      _ => a,
    }
  }

  pub fn style(&self) -> Style {
    Style::default().fg(self.color("main_fg")).bg(self.color("main_bg"))
  }
}

impl Default for Theme {
  fn default() -> Self {
    let mut colors = HashMap::new();
    for (key, rgb) in [
      ("main_bg", (0, 0, 0)),
      ("main_fg", (204, 204, 204)),
      ("title", (238, 238, 238)),
      ("hi_fg", (181, 64, 64)),
      ("selected_bg", (106, 47, 47)),
      ("selected_fg", (238, 238, 238)),
      ("inactive_fg", (64, 64, 64)),
      ("graph_text", (96, 96, 96)),
      ("meter_bg", (64, 64, 64)),
      ("cpu_box", (85, 109, 89)),
      ("mem_box", (108, 108, 75)),
      ("net_box", (92, 88, 141)),
      ("proc_box", (128, 82, 82)),
      ("div_line", (48, 48, 48)),
      ("proc_misc", (13, 231, 86)),
      ("proc_pause_bg", (181, 64, 64)),
      ("proc_follow_bg", (64, 64, 181)),
      ("proc_banner_bg", (123, 64, 123)),
      ("proc_banner_fg", (238, 238, 238)),
      ("followed_bg", (64, 64, 181)),
      ("followed_fg", (238, 238, 238)),
    ] {
      colors.insert(key.to_owned(), Color::Rgb(rgb.0, rgb.1, rgb.2));
    }
    for (name, stops) in [
      ("cpu", [(119, 202, 155), (203, 192, 108), (220, 76, 76)]),
      ("temp", [(72, 151, 212), (84, 116, 232), (255, 64, 182)]),
      ("free", [(56, 79, 33), (181, 230, 133), (220, 255, 133)]),
      ("cached", [(22, 51, 80), (116, 230, 252), (38, 197, 255)]),
      ("available", [(78, 63, 14), (255, 215, 122), (255, 184, 20)]),
      ("used", [(89, 43, 38), (217, 98, 109), (255, 71, 105)]),
      ("download", [(41, 31, 117), (79, 67, 163), (176, 169, 222)]),
      ("upload", [(98, 6, 101), (125, 65, 128), (220, 175, 222)]),
      ("process", [(128, 208, 163), (220, 209, 121), (212, 84, 84)]),
    ] {
      for (suffix, (r, g, b)) in ["start", "mid", "end"].into_iter().zip(stops) {
        colors.insert(format!("{name}_{suffix}"), Color::Rgb(r, g, b));
      }
    }
    Self(colors)
  }
}

fn parse_color(value: &str) -> Option<Color> {
  if value.is_empty() {
    return Some(Color::Reset);
  }
  if let Some(hex) = value.strip_prefix('#') {
    if !hex.is_ascii() {
      return None;
    }
    return match hex.len() {
      2 => u8::from_str_radix(hex, 16).ok().map(|v| Color::Rgb(v, v, v)),
      6 => Some(Color::Rgb(
        u8::from_str_radix(&hex[0..2], 16).ok()?,
        u8::from_str_radix(&hex[2..4], 16).ok()?,
        u8::from_str_radix(&hex[4..6], 16).ok()?,
      )),
      _ => None,
    };
  }
  let values: Vec<_> = value
    .split_whitespace()
    .map(str::parse::<i32>)
    .collect::<Result<_, _>>()
    .ok()?;
  match values.as_slice() {
    [r, g, b] => Some(Color::Rgb(
      (*r).clamp(0, 255) as u8,
      (*g).clamp(0, 255) as u8,
      (*b).clamp(0, 255) as u8,
    )),
    _ => None,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod theme {
    use super::*;

    mod parse {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_reads_btop_colors_and_optional_gradient_stops() {
        let theme = Theme::parse("# theme\n theme[main_bg]=\"\"\ntheme[main_fg]=\"#cc\"\ntheme[cpu_start]=\"0 10 20\"\ntheme[cpu_mid]=\"\"\ntheme[cpu_end]=\"#64788c\"\ntheme[future_key]=\"ignored\"").unwrap();
        assert_eq!(theme.color("main_bg"), Color::Reset);
        assert_eq!(theme.color("main_fg"), Color::Rgb(204, 204, 204));
        assert_eq!(theme.gradient("cpu", 0.5), Color::Rgb(50, 65, 80));
        assert_eq!(theme.gradient("process", 0.5), theme.gradient("cpu", 0.5));
      }

      #[test]
      fn it_rejects_invalid_colors_without_panicking_on_unicode() {
        for value in ["#éaaaa", "#xyzxyz", "1 2", "\"unterminated"] {
          assert!(Theme::parse(&format!("theme[main_fg]={value}")).is_err());
        }
      }
    }
  }
}
