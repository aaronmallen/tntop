use std::{path::PathBuf, process::ExitCode};

use clap::Parser;
use getset::{CopyGetters, Getters};
use log::LevelFilter;

use crate::{config::Settings, logger::Logger, ui::Banner};

#[derive(CopyGetters, Debug, Getters, Parser)]
#[clap(about, before_help = Banner::new().with_author().with_version().to_string(), disable_version_flag = true, name = "tntop")]
pub struct App {
  /// Path to a configuration file
  #[get = "pub"]
  #[arg(global = true, long = "config", short = 'c')]
  config_path: Option<PathBuf>,
  /// Start in debug mode with additional logs and metrics
  #[get_copy = "pub"]
  #[arg(global = true, long = "debug", short = 'd')]
  debug_mode: bool,
  /// Show version information and exit
  #[get_copy = "pub"]
  #[arg(long = "version", short = 'V')]
  print_version: bool,
  /// Open the configured server with this name
  #[arg(long)]
  server: Option<String>,
  /// Start with a configured preset number
  #[arg(long)]
  preset: Option<usize>,
}

impl App {
  pub async fn call(&self, context: Context) -> Result<()> {
    crate::ui::run(
      context.settings(),
      context.config_path(),
      self.server.as_deref(),
      self.preset,
    )
    .await?;
    Ok(())
  }
}

#[derive(Clone, Debug, Getters)]
pub struct Context {
  #[get = "pub"]
  config_path: PathBuf,
  #[get = "pub"]
  settings: Settings,
}

impl Context {
  pub fn new(config_path: PathBuf, settings: Settings) -> Self {
    Self {
      config_path,
      settings,
    }
  }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error(transparent)]
  Client(#[from] crate::client::Error),
  #[error(transparent)]
  Configuration(#[from] crate::config::Error),
  #[error(transparent)]
  Credentials(#[from] crate::credentials::Error),
  #[error(transparent)]
  Logging(#[from] log::SetLoggerError),
  #[error(transparent)]
  Ui(#[from] crate::ui::Error),
}

impl From<Error> for ExitCode {
  fn from(error: Error) -> Self {
    use crate::{client, config, credentials, ui};

    match error {
      Error::Client(client::Error::Authentication) => Self::from(77),
      Error::Client(client::Error::CaCertificate {
        ..
      }) => Self::from(78),
      Error::Client(_) => Self::from(69),
      Error::Configuration(config::Error::Io(_) | config::Error::Persist(_)) => Self::from(74),
      Error::Configuration(_) => Self::from(78),
      Error::Credentials(credentials::Error::Join(_)) | Error::Logging(_) => Self::from(70),
      Error::Credentials(_) => Self::from(77),
      Error::Ui(ui::Error::Client(error)) => Error::Client(error).into(),
      Error::Ui(ui::Error::Configuration(error)) => Error::Configuration(error).into(),
      Error::Ui(ui::Error::Credentials(error)) => Error::Credentials(error).into(),
      Error::Ui(ui::Error::Io(_)) => Self::from(74),
      Error::Ui(ui::Error::NoTerminal) => Self::from(2),
      Error::Ui(ui::Error::Theme(_) | ui::Error::UnknownServer(_)) => Self::from(78),
    }
  }
}

type Result<T> = std::result::Result<T, Error>;

pub async fn run() -> Result<()> {
  let app = App::parse();

  setup_logging(app.debug_mode())?;

  if app.print_version() {
    anstream::println!("{}", Banner::new().with_author().with_version());
    return Ok(());
  }

  let config_path = match app.config_path() {
    Some(path) => path,
    None => &crate::config::default_path()?,
  };

  let settings = crate::config::load(config_path)?;

  let context = Context::new(config_path.clone(), settings);

  app.call(context).await
}

fn setup_logging(debug_mode: bool) -> Result<()> {
  log::set_logger(&Logger)?;

  if debug_mode {
    log::set_max_level(LevelFilter::Trace);
  } else {
    log::set_max_level(LevelFilter::Warn);
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  mod exit_code {
    use super::*;

    mod from_error {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_keeps_the_cause_code_through_credential_and_ui_wrappers() {
        use crate::{client, config, credentials, ui};

        for (error, expected) in [
          (client::Error::Authentication, 77),
          (client::Error::Closed, 69),
          (
            client::Error::CaCertificate {
              path: "nas.pem".into(),
              reason: "invalid certificate".into(),
            },
            78,
          ),
        ] {
          assert_eq!(
            ExitCode::from(Error::Ui(ui::Error::Client(error))),
            ExitCode::from(expected)
          );
        }
        assert_eq!(
          ExitCode::from(Error::Credentials(credentials::Error::Empty)),
          ExitCode::from(77)
        );
        assert_eq!(
          ExitCode::from(Error::Ui(ui::Error::Configuration(config::Error::Invalid(
            "bad pin".into()
          )))),
          ExitCode::from(78)
        );
        assert_eq!(
          ExitCode::from(Error::Ui(ui::Error::Io(std::io::Error::other("read failed")))),
          ExitCode::from(74)
        );
      }
    }
  }
}
