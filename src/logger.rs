use std::io::{self, Write};

use log::Log;

pub struct Logger;

impl Log for Logger {
  fn enabled(&self, metadata: &log::Metadata) -> bool {
    metadata.target().starts_with("tntop") && metadata.level() <= log::max_level()
  }

  fn flush(&self) {
    let _ = io::stderr().flush();
  }

  fn log(&self, record: &log::Record) {
    if self.enabled(record.metadata()) {
      let _ = writeln!(
        io::stderr().lock(),
        "{} {}: {}",
        record.level(),
        record.target(),
        record.args()
      );
    }
  }
}
