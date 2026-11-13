mod cli;
mod client;
mod config;
mod credentials;
mod logger;
mod ui;

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
  if let Err(e) = cli::run().await {
    eprintln!("ERROR: {e}");
    return e.into();
  }

  ExitCode::SUCCESS
}
