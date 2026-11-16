use std::fmt;

use anstyle::{RgbColor, Style};
use ratatui::{
  style::{Color as TuiColor, Style as TuiStyle},
  text::{Line, Span, Text},
};

const AUTHOR: &str = "@aaronmallen";

const AUTHOR_COLOR: RgbColor = RgbColor(0x3D, 0xFF, 0xB0);

const AUTHOR_PREFIX: &str = "by ";

const AUTHOR_PREFIX_COLOR: RgbColor = RgbColor(0xFF, 0xFF, 0xFF);

const BANNER: &str = concat!(
  "\n",
  "████████╗███╗   ██╗████████╗ ██████╗ ██████╗\n",
  "╚══██╔══╝████╗  ██║╚══██╔══╝██╔═══██╗██╔══██╗\n",
  "   ██║   ██╔██╗ ██║   ██║   ██║   ██║██████╔╝\n",
  "   ██║   ██║╚██╗██║   ██║   ██║   ██║██╔═══╝\n",
  "   ██║   ██║ ╚████║   ██║   ╚██████╔╝██║\n",
  "   ╚═╝   ╚═╝  ╚═══╝   ╚═╝    ╚═════╝ ╚═╝\n",
);

const GRADIENT_EASING: f32 = 2.2;

const GRADIENT_ROW_WEIGHT: f32 = 0.6;

const GRADIENT_STOPS: [RgbColor; 6] = [
  RgbColor(0x91, 0x18, 0xFE),
  RgbColor(0xA7, 0x19, 0xEE),
  RgbColor(0xBD, 0x1B, 0xDE),
  RgbColor(0xD3, 0x1C, 0xCF),
  RgbColor(0xE9, 0x1D, 0xBF),
  RgbColor(0xFF, 0x1F, 0xAF),
];

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug, Default)]
pub struct Banner {
  with_author: bool,
  with_version: bool,
}

impl Banner {
  pub fn new() -> Self {
    Self::default()
  }

  fn cells() -> Vec<Vec<(char, RgbColor)>> {
    let last_row = Self::rows().count().saturating_sub(1).max(1) as f32;
    let last_col = Self::width().saturating_sub(1).max(1) as f32;

    Self::rows()
      .enumerate()
      .map(|(row, text)| {
        text
          .chars()
          .enumerate()
          .map(|(col, glyph)| {
            let position =
              (row as f32 / last_row) * GRADIENT_ROW_WEIGHT + (col as f32 / last_col) * (1.0 - GRADIENT_ROW_WEIGHT);
            (glyph, gradient(position.powf(GRADIENT_EASING)))
          })
          .collect()
      })
      .collect()
  }

  fn rows() -> impl Iterator<Item = &'static str> {
    BANNER.lines().filter(|row| !row.is_empty())
  }

  fn width() -> usize {
    Self::rows().map(|row| row.chars().count()).max().unwrap_or(0)
  }

  pub fn with_author(mut self) -> Self {
    self.with_author = true;
    self
  }

  pub fn with_version(mut self) -> Self {
    self.with_version = true;
    self
  }

  fn footer(&self) -> Option<Footer> {
    let version = self.with_version.then(|| format!("v{VERSION}"));
    let author = self.with_author.then_some(AUTHOR);

    if version.is_none() && author.is_none() {
      return None;
    }

    let used = version.as_deref().map_or(0, |v| v.chars().count())
      + author.map_or(0, |a| AUTHOR_PREFIX.chars().count() + a.chars().count());

    Some(Footer {
      author,
      gap: Self::width().saturating_sub(used),
      version,
    })
  }
}

impl fmt::Display for Banner {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for (index, row) in Self::cells().into_iter().enumerate() {
      if index > 0 {
        writeln!(f)?;
      }
      for (glyph, color) in row {
        if glyph == ' ' {
          write!(f, " ")?;
        } else {
          let style = Style::new().fg_color(Some(color.into()));
          write!(f, "{style}{glyph}{style:#}")?;
        }
      }
    }

    let Some(footer) = self.footer() else {
      return Ok(());
    };

    writeln!(f)?;

    if let Some(version) = &footer.version {
      write!(f, "{version}")?;
    }

    write!(f, "{:width$}", "", width = footer.gap)?;

    if let Some(author) = footer.author {
      let prefix_style = Style::new().fg_color(Some(AUTHOR_PREFIX_COLOR.into()));
      let author_style = Style::new().fg_color(Some(AUTHOR_COLOR.into())).italic();
      write!(
        f,
        "{prefix_style}{AUTHOR_PREFIX}{prefix_style:#}{author_style}{author}{author_style:#}"
      )?;
    }

    Ok(())
  }
}

impl From<&Banner> for Text<'static> {
  fn from(banner: &Banner) -> Self {
    let mut lines: Vec<Line<'static>> = Banner::cells()
      .into_iter()
      .map(|row| {
        let spans: Vec<Span<'static>> = row
          .into_iter()
          .map(|(glyph, color)| Span::styled(glyph.to_string(), tui_color(color)))
          .collect();
        Line::from(spans)
      })
      .collect();

    if let Some(footer) = banner.footer() {
      let version = footer.version.map(Span::raw);
      let gap = Span::raw(" ".repeat(footer.gap));
      let author = footer.author.map(|a| {
        [
          Span::styled(AUTHOR_PREFIX, TuiStyle::new().fg(tui_color(AUTHOR_PREFIX_COLOR))),
          Span::styled(a, TuiStyle::new().fg(tui_color(AUTHOR_COLOR)).italic()),
        ]
      });

      let spans: Vec<Span<'static>> = version
        .into_iter()
        .chain([gap])
        .chain(author.into_iter().flatten())
        .collect();

      lines.push(Line::from(spans));
    }

    Text::from(lines)
  }
}

struct Footer {
  author: Option<&'static str>,
  gap: usize,
  version: Option<String>,
}

fn gradient(position: f32) -> RgbColor {
  let segments = GRADIENT_STOPS.len() - 1;
  let scaled = position.clamp(0.0, 1.0) * segments as f32;
  let index = (scaled.floor() as usize).min(segments - 1);
  let local = scaled - index as f32;
  let from = GRADIENT_STOPS[index];
  let to = GRADIENT_STOPS[index + 1];
  let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * local).round() as u8;

  RgbColor(mix(from.r(), to.r()), mix(from.g(), to.g()), mix(from.b(), to.b()))
}

fn tui_color(color: RgbColor) -> TuiColor {
  TuiColor::Rgb(color.r(), color.g(), color.b())
}

#[cfg(test)]
mod tests {
  use super::*;

  mod banner {
    use super::*;

    mod display {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_keeps_terminal_and_tui_text_and_footer_alignment_in_sync() {
        for (author, version) in [(false, false), (true, false), (false, true), (true, true)] {
          let mut banner = Banner::new();
          if author {
            banner = banner.with_author();
          }
          if version {
            banner = banner.with_version();
          }
          let ansi = banner.to_string();
          let mut plain = String::new();
          let mut escape = false;
          for character in ansi.chars() {
            if character == '\u{1b}' {
              escape = true;
            } else if escape {
              if character == 'm' {
                escape = false;
              }
            } else {
              plain.push(character);
            }
          }
          let text = Text::from(&banner);
          assert_eq!(plain, text.to_string());
          assert_eq!(text.lines.len(), if author || version { 7 } else { 6 });
          assert!(plain.starts_with("████████╗"));
          if author || version {
            let footer = plain.lines().last().unwrap();
            let width = plain.lines().take(6).map(|line| line.chars().count()).max().unwrap();
            assert_eq!(footer.chars().count(), width);
            assert_eq!(footer.contains("@aaronmallen"), author);
            assert_eq!(footer.starts_with(&format!("v{}", env!("CARGO_PKG_VERSION"))), version);
          }
          let colors: std::collections::HashSet<_> = text.lines[..6]
            .iter()
            .flat_map(|line| &line.spans)
            .filter_map(|span| span.style.fg)
            .collect();
          assert!(colors.len() > 6, "the banner should blend its gradient across cells");
          if author {
            let span = text.lines.last().unwrap().spans.last().unwrap();
            assert!(span.style.add_modifier.contains(ratatui::style::Modifier::ITALIC));
          }
        }
      }
    }
  }
}
