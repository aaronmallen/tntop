use ratatui::{Frame, crossterm::event::KeyCode, layout::Rect, text::Line, widgets::Paragraph};

pub(super) struct Input {
  pub value: String,
  cursor: usize,
}

impl Default for Input {
  fn default() -> Self {
    Self::new("")
  }
}

impl Input {
  pub fn new(value: impl Into<String>) -> Self {
    let value = value.into();
    Self {
      cursor: value.len(),
      value,
    }
  }

  pub fn handle(&mut self, key: KeyCode) {
    match key {
      KeyCode::Left => {
        self.cursor = self.value[..self.cursor]
          .char_indices()
          .next_back()
          .map_or(0, |(i, _)| i)
      }
      KeyCode::Right => {
        if let Some(c) = self.value[self.cursor..].chars().next() {
          self.cursor += c.len_utf8();
        }
      }
      KeyCode::Home => self.cursor = 0,
      KeyCode::End => self.cursor = self.value.len(),
      KeyCode::Backspace => {
        let end = self.cursor;
        self.handle(KeyCode::Left);
        self.value.drain(self.cursor..end);
      }
      KeyCode::Delete if self.cursor < self.value.len() => {
        self.value.remove(self.cursor);
      }
      KeyCode::Char(c) if !c.is_control() => {
        self.value.insert(self.cursor, c);
        self.cursor += c.len_utf8();
      }
      _ => {}
    }
  }

  pub fn draw(&self, frame: &mut Frame, area: Rect, focused: bool) {
    if area.is_empty() {
      return;
    }
    let mut start = 0;
    while Line::from(&self.value[start..self.cursor]).width() >= area.width as usize {
      start += self.value[start..].chars().next().unwrap().len_utf8();
    }
    frame.render_widget(Paragraph::new(&self.value[start..]), area);
    if focused {
      frame.set_cursor_position((
        area.x + Line::from(&self.value[start..self.cursor]).width() as u16,
        area.y,
      ));
    }
  }
}

#[cfg(test)]
mod tests {
  use ratatui::{Terminal, backend::TestBackend};

  use super::*;

  #[test]
  fn it_edits_unicode_in_the_middle_and_respects_boundaries() {
    let mut input = Input::new("a界é");
    input.handle(KeyCode::Left);
    input.handle(KeyCode::Backspace);
    input.handle(KeyCode::Char('ß'));
    assert_eq!(input.value, "aßé");
    input.handle(KeyCode::Delete);
    assert_eq!(input.value, "aß");
    input.handle(KeyCode::End);
    input.handle(KeyCode::Right);
    input.handle(KeyCode::Delete);
    assert_eq!(input.cursor, input.value.len());
    input.handle(KeyCode::Home);
    input.handle(KeyCode::Left);
    input.handle(KeyCode::Backspace);
    input.handle(KeyCode::Char('界'));
    assert_eq!(input.value, "界aß");
    assert_eq!(input.cursor, 3);
  }

  #[test]
  fn it_keeps_the_cursor_visible_in_narrow_fields() {
    let mut terminal = Terminal::new(TestBackend::new(5, 1)).unwrap();
    let input = Input::new("some long 界 value");
    terminal.draw(|frame| input.draw(frame, frame.area(), true)).unwrap();
    let cursor = terminal.get_cursor_position().unwrap();
    assert!(cursor.x < 5);
    assert_eq!(cursor.y, 0);
    assert_eq!(terminal.backend().buffer()[(0, 0)].symbol(), "a");
  }
}
