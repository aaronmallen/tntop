use std::{
  fs::File,
  io::{self, Write},
  path::Path,
  sync::Mutex,
};

use log::Log;

pub struct Logger;

static OUTPUT: Mutex<Option<File>> = Mutex::new(None);

impl Logger {
  pub fn redirect(path: &Path, level: log::LevelFilter) -> io::Result<impl Drop + use<>> {
    let file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    let output = Output {
      file: OUTPUT.lock().unwrap().replace(file),
      level: log::max_level(),
    };
    log::set_max_level(level);
    Ok(output)
  }

  pub fn set_level(level: log::LevelFilter) {
    if OUTPUT.lock().unwrap().is_some() {
      log::set_max_level(level);
    }
  }
}

struct Output {
  file: Option<File>,
  level: log::LevelFilter,
}

impl Drop for Output {
  fn drop(&mut self) {
    *OUTPUT.lock().unwrap() = self.file.take();
    log::set_max_level(self.level);
  }
}

impl Log for Logger {
  fn enabled(&self, metadata: &log::Metadata) -> bool {
    metadata.target().starts_with("tntop") && metadata.level() <= log::max_level()
  }

  fn flush(&self) {
    if let Some(file) = OUTPUT.lock().unwrap().as_mut() {
      let _ = file.flush();
    } else {
      let _ = io::stderr().flush();
    }
  }

  fn log(&self, record: &log::Record) {
    if self.enabled(record.metadata()) {
      let mut output = OUTPUT.lock().unwrap();
      let mut stderr = io::stderr().lock();
      let writer: &mut dyn Write = match output.as_mut() {
        Some(file) => file,
        None => &mut stderr,
      };
      let _ = writeln!(writer, "{} {}: {}", record.level(), record.target(), record.args());
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod redirect {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_writes_only_enabled_diagnostics_to_file_and_restores_the_prior_level() {
      let directory = tempfile::tempdir().unwrap();
      let path = directory.path().join("tntop.log");
      let prior = log::max_level();
      {
        let _output = Logger::redirect(&path, log::LevelFilter::Warn).unwrap();
        Logger.log(
          &log::Record::builder()
            .target("tntop::client")
            .level(log::Level::Warn)
            .args(format_args!("visible"))
            .build(),
        );
        Logger.log(
          &log::Record::builder()
            .target("tntop::client")
            .level(log::Level::Debug)
            .args(format_args!("hidden"))
            .build(),
        );
        Logger::set_level(log::LevelFilter::Debug);
        Logger.log(
          &log::Record::builder()
            .target("tntop::client")
            .level(log::Level::Debug)
            .args(format_args!("enabled"))
            .build(),
        );
      }
      assert_eq!(log::max_level(), prior);
      assert!(OUTPUT.lock().unwrap().is_none());
      let text = std::fs::read_to_string(&path).unwrap();
      assert!(text.contains("visible"));
      assert!(text.contains("enabled"));
      assert!(!text.contains("hidden"));
      assert!(Logger::redirect(directory.path(), log::LevelFilter::Trace).is_err());
      assert_eq!(log::max_level(), prior);
    }
  }
}
