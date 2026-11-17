use ratatui::{
  Frame,
  crossterm::event::KeyCode,
  layout::Rect,
  text::Line,
  widgets::{Clear, Paragraph},
};

use super::{theme::Theme, view::panel};

fn lines() -> Vec<&'static str> {
  vec![
    "Navigation",
    "  m / Esc           Main menu; Esc first closes the active layer",
    "  F2 / o            Options",
    "  F1 / ? / h / H    Help",
    "  Vim keys          h/j/k/l move left/down/up/right when enabled",
    "                    Use H for Help; text fields keep literal letters",
    "  s                 Server manager",
    "  q / Ctrl-C        Quit; Ctrl-C works inside editors too",
    "  Ctrl-R            Reload display preferences",
    "  + / = / -         Increase / decrease redraw interval by 100 ms",
    "                    Holding + or - accelerates to 1000 ms steps",
    "                    Collection stays at least 2000 ms",
    "  Main > Update     Check and install the latest stable release",
    "",
    "Options",
    "  Tab / Shift-Tab   Next / previous category",
    "  1–5               Select a category",
    "  Left / Right      Change the selected value",
    "  Up / Down         Select a setting, wrapping at the ends",
    "  Page Up / Down    Previous / next visible page",
    "  Home / End        First / last setting",
    "  Enter / Space     Toggle a boolean or edit a value",
    "  Enter / Esc       Accept / cancel an edit",
    "  Esc / q / o       Close Options when not editing",
    "",
    "Server and Options text fields",
    "  Left / Right      Move the cursor",
    "  Home / End        Start / end of the field",
    "  Backspace/Delete  Remove before / after the cursor",
    "  Typing inserts at the cursor. Long fields scroll horizontally.",
    "  Editing consumes shortcuts before dashboard commands.",
    "",
    "Servers",
    "  Up / Down         Select a server",
    "  Enter             Open the selected server",
    "  a / d             Add / delete a server",
    "  e / t             Change API key / review HTTPS certificate",
    "  Tab / Up / Down   Switch fields in the add-server dialog",
    "",
    "Dashboard",
    "  1 / 2 / 3 / 4     Toggle CPU / Memory / Network / Apps",
    "  p / P             Next / previous enabled preset",
    "  Border preset     Click to cycle; * means a changed layout",
    "  Up / Down         Select an app",
    "  Page Up / Down    Page through apps",
    "  Home / End        First / last app",
    "  Enter             Open / close app details",
    "  F                 Pin / unpin details to the selected app",
    "  u / Space         Pause / resume only app presentation",
    "  c / %             Toggle per-core CPU / memory bytes",
    "  i / d             Toggle storage I/O view / show storage",
    "  Left / Right      Change app sort order",
    "  r                 Reverse app sort order",
    "  f / /             Edit the app filter with cursor movement",
    "  Enter / Down      Accept the app filter",
    "  Esc               Restore the filter from before editing",
    "  Delete            Clear the filter outside text editing",
    "  b / n             Previous / next network interface",
    "  a / y             Toggle automatic / synchronized network scales",
    "  z                 Reset displayed totals; press again for full session",
    "  [ / ]             Previous / next storage pool",
    "",
    "Help",
    "  Up / Down         Scroll one line",
    "  Page Up / Down    Scroll a page",
    "  Home / End        First / last line",
    "  Esc / q / h       Close Help; return to where it opened",
    "  Enter / Space     Close Help (Backspace or click also works)",
    "",
    "Mouse",
    "  Click menu entries, settings, categories and footer controls.",
    "  Click a server to open it, or an app row to select it.",
    "  Click the selected app row again to toggle its details.",
    "  Scroll over lists and Help. Disable mouse in General options.",
    "",
    "HTTP sends API keys and server data without encryption.",
    "Disk I/O is aggregate. Network totals are session estimates.",
  ]
}

pub(super) fn handle(offset: &mut usize, key: KeyCode) -> bool {
  match key {
    KeyCode::Up => *offset = offset.saturating_sub(1),
    KeyCode::Down => *offset = (*offset + 1).min(lines().len() - 1),
    KeyCode::PageUp => *offset = offset.saturating_sub(15),
    KeyCode::PageDown => *offset = (*offset + 15).min(lines().len() - 1),
    KeyCode::Home => *offset = 0,
    KeyCode::End => *offset = lines().len() - 1,
    _ => return false,
  }
  true
}

pub(super) fn draw(frame: &mut Frame, offset: &mut usize, theme: &Theme) -> Rect {
  let bounds = frame.area();
  let width = bounds.width.min(90);
  let height = bounds.height.min(30);
  let area = Rect::new(
    bounds.x + (bounds.width - width) / 2,
    bounds.y + (bounds.height - height) / 2,
    width,
    height,
  );
  let lines = lines();
  *offset = (*offset).min(lines.len().saturating_sub(height.saturating_sub(2) as usize));
  frame.render_widget(Clear, area);
  frame.render_widget(
    Paragraph::new(lines.into_iter().map(Line::from).collect::<Vec<_>>())
      .style(theme.style())
      .block(panel(" help: ↑↓ / PgUp/PgDn / Esc ", "cpu_box", theme))
      .scroll((*offset as u16, 0)),
    area,
  );
  area
}

#[cfg(test)]
mod tests {
  use ratatui::{Terminal, backend::TestBackend};

  use super::*;

  #[test]
  fn it_pages_and_clamps_after_resizing() {
    let mut offset = 0;
    assert!(handle(&mut offset, KeyCode::PageDown));
    assert_eq!(offset, 15);
    handle(&mut offset, KeyCode::End);
    for (width, height) in [(70, 24), (20, 5), (100, 40)] {
      let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
      terminal
        .draw(|frame| {
          draw(frame, &mut offset, &Theme::default());
        })
        .unwrap();
      assert!(offset < lines().len());
    }
    handle(&mut offset, KeyCode::Home);
    handle(&mut offset, KeyCode::Up);
    assert_eq!(offset, 0);
    assert!(!handle(&mut offset, KeyCode::Esc));
  }
}
