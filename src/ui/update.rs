use std::{
  fs, io,
  path::Path,
  process::{Command, Stdio},
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  time::{Duration, Instant},
};

use serde::Deserialize;

#[derive(Default)]
pub(super) struct Update {
  task: Option<tokio::task::JoinHandle<Result<String, Error>>>,
  message: String,
  cancelled: Arc<AtomicBool>,
}

impl Update {
  pub fn start(&mut self) {
    if self.task.is_none() {
      self.message = "Checking releases and installing an update if available...".into();
      self.cancelled.store(false, Ordering::Relaxed);
      let cancelled = self.cancelled.clone();
      self.task = Some(tokio::task::spawn_blocking(move || run(&cancelled)));
    }
  }

  pub async fn poll(&mut self) -> bool {
    if !self.task.as_ref().is_some_and(|task| task.is_finished()) {
      return false;
    }
    self.message = match self.task.take().unwrap().await {
      Ok(Ok(message)) => message,
      Ok(Err(error)) => error.to_string(),
      Err(error) => format!("Update failed: {error}"),
    };
    true
  }

  pub fn message(&self) -> &str {
    &self.message
  }

  pub async fn stop(&mut self) {
    self.cancelled.store(true, Ordering::Relaxed);
    if let Some(task) = self.task.take() {
      let _ = task.await;
    }
  }
}

impl Drop for Update {
  fn drop(&mut self) {
    self.cancelled.store(true, Ordering::Relaxed);
  }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("self-update: {0}")]
  Message(String),
  #[error("self-update: {0}")]
  Io(#[from] io::Error),
  #[error("self-update: invalid release response: {0}")]
  Json(#[from] serde_json::Error),
}

#[derive(Deserialize)]
struct Release {
  tag_name: String,
  draft: bool,
  prerelease: bool,
  assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
  name: String,
}

fn version(value: &str) -> Option<[u64; 3]> {
  let parts = value.split('.').map(str::parse).collect::<Result<Vec<u64>, _>>().ok()?;
  parts.try_into().ok()
}

fn target(os: &str, arch: &str) -> Result<&'static str, Error> {
  match (os, arch) {
    ("macos", "aarch64") => Ok("aarch64-apple-darwin"),
    ("linux", "x86_64") => Ok("x86_64-unknown-linux-gnu"),
    ("windows", "x86_64") => Ok("x86_64-pc-windows-gnu"),
    _ => Err(Error::Message(format!("unsupported platform: {os}/{arch}"))),
  }
}

fn latest(releases: impl IntoIterator<Item = Release>) -> Option<Release> {
  releases
    .into_iter()
    .filter(|release| !release.draft && !release.prerelease && version(&release.tag_name).is_some())
    .max_by_key(|release| version(&release.tag_name))
}

fn command(command: &mut Command, cancelled: &AtomicBool, limit: Duration) -> Result<(), Error> {
  if cancelled.load(Ordering::Relaxed) {
    return Err(Error::Message("cancelled".into()));
  }
  let errors = tempfile::NamedTempFile::new()?;
  let mut child = command.stdout(Stdio::null()).stderr(errors.reopen()?).spawn()?;
  let deadline = Instant::now() + limit;
  let result = loop {
    if cancelled.load(Ordering::Relaxed) {
      break Err(Error::Message("cancelled".into()));
    }
    if Instant::now() >= deadline {
      break Err(Error::Message("command timed out".into()));
    }
    match child.try_wait() {
      Ok(Some(status)) if status.success() => break Ok(()),
      Ok(Some(_)) => {
        break Err(Error::Message(format!(
          "command failed: {}",
          fs::read_to_string(errors.path())?.trim()
        )));
      }
      Ok(None) => std::thread::sleep(Duration::from_millis(20)),
      Err(error) => break Err(error.into()),
    }
  };
  if result.is_err() {
    let _ = child.kill();
    child.wait()?;
  }
  result
}

fn download(url: &str, destination: &Path, cancelled: &AtomicBool) -> Result<(), Error> {
  command(
    Command::new(if cfg!(windows) { "curl.exe" } else { "curl" })
      .args([
        "--fail",
        "--location",
        "--silent",
        "--show-error",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
        "--tlsv1.2",
        "--max-time",
        "120",
        "--output",
      ])
      .arg(destination)
      .arg(url),
    cancelled,
    Duration::from_secs(120),
  )
}

fn verify(archive: &Path, checksums: &str, name: &str) -> Result<(), Error> {
  let matches: Vec<_> = checksums
    .lines()
    .filter_map(|line| {
      let mut fields = line.split_whitespace();
      let checksum = fields.next()?;
      let filename = fields.next()?.trim_start_matches('*');
      (filename == name && fields.next().is_none()).then_some(checksum)
    })
    .collect();
  let [expected] = matches.as_slice() else {
    return Err(Error::Message(format!("missing or duplicate checksum for {name}")));
  };
  let mut file = fs::File::open(archive)?;
  let mut hash = ring::digest::Context::new(&ring::digest::SHA256);
  let mut buffer = [0; 65536];
  loop {
    use io::Read;
    let count = file.read(&mut buffer)?;
    if count == 0 {
      break;
    }
    hash.update(&buffer[..count]);
  }
  let actual: String = hash
    .finish()
    .as_ref()
    .iter()
    .map(|byte| format!("{byte:02x}"))
    .collect();
  if !actual.eq_ignore_ascii_case(expected) {
    return Err(Error::Message(format!("checksum mismatch for {name}")));
  }
  Ok(())
}

#[cfg(not(windows))]
fn install(source: &Path, destination: &Path) -> Result<(), Error> {
  use std::os::unix::fs::PermissionsExt;
  let parent = destination
    .parent()
    .ok_or_else(|| Error::Message("executable has no parent directory".into()))?;
  let staged = tempfile::NamedTempFile::new_in(parent)?;
  fs::copy(source, staged.path())?;
  fs::set_permissions(staged.path(), fs::Permissions::from_mode(0o755))?;
  staged.persist(destination).map_err(|error| Error::Io(error.error))?;
  Ok(())
}

#[cfg(windows)]
fn install(source: &Path, _destination: &Path) -> Result<(), Error> {
  self_replace::self_replace(source)?;
  Ok(())
}

fn run(cancelled: &AtomicBool) -> Result<String, Error> {
  let target = target(std::env::consts::OS, std::env::consts::ARCH)?;
  let temporary = tempfile::tempdir()?;
  let metadata = temporary.path().join("releases.json");
  let mut releases = Vec::new();
  for page in 1.. {
    download(
      &format!("https://aaronmallen.dev/api/v1/repos/aaron/tntop/releases?limit=50&page={page}"),
      &metadata,
      cancelled,
    )?;
    let rows: Vec<Release> = serde_json::from_slice(&fs::read(&metadata)?)?;
    let done = rows.len() < 50;
    releases.extend(rows);
    if done {
      break;
    }
  }
  let release = latest(releases).ok_or_else(|| Error::Message("no stable release is available".into()))?;
  let installed = env!("CARGO_PKG_VERSION");
  let current = version(installed).ok_or_else(|| Error::Message("installed version is not a stable version".into()))?;
  if version(&release.tag_name).unwrap() <= current {
    return Ok(format!(
      "Installed: {installed}. Available: {}. Already current or newer.",
      release.tag_name
    ));
  }
  let stem = format!("tntop-{}-{target}", release.tag_name);
  let name = format!("{stem}.{}", if cfg!(windows) { "zip" } else { "tar.gz" });
  for required in [&*name, "SHA256SUMS"] {
    if !release.assets.iter().any(|asset| asset.name == required) {
      return Err(Error::Message(format!(
        "release {} has no {required}",
        release.tag_name
      )));
    }
  }
  let base = format!(
    "https://aaronmallen.dev/aaron/tntop/releases/download/{}",
    release.tag_name
  );
  let archive = temporary.path().join(&name);
  let checksums = temporary.path().join("SHA256SUMS");
  download(&format!("{base}/{name}"), &archive, cancelled)?;
  download(&format!("{base}/SHA256SUMS"), &checksums, cancelled)?;
  verify(&archive, &fs::read_to_string(checksums)?, &name)?;
  let executable = if cfg!(windows) { "tntop.exe" } else { "tntop" };
  let member = format!("{stem}/{executable}");
  command(
    Command::new("tar")
      .arg("-xf")
      .arg(&archive)
      .arg("-C")
      .arg(temporary.path())
      .arg(&member),
    cancelled,
    Duration::from_secs(30),
  )?;
  let binary = temporary.path().join(member);
  if !fs::symlink_metadata(&binary)?.file_type().is_file() {
    return Err(Error::Message("archive does not contain a regular executable".into()));
  }
  if cancelled.load(Ordering::Relaxed) {
    return Err(Error::Message("cancelled".into()));
  }
  install(&binary, &std::env::current_exe()?)?;
  Ok(format!("Updated tntop from {installed} to {}.", release.tag_name))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[cfg(unix)]
  #[test]
  fn it_bounds_commands_and_reaps_them_on_timeout() {
    let directory = tempfile::tempdir().unwrap();
    let pid = directory.path().join("pid");
    let mut process = Command::new("sh");
    process
      .arg("-c")
      .arg("echo $$ > \"$1\"; exec sleep 30")
      .arg("sh")
      .arg(&pid);
    let started = Instant::now();
    let error = command(&mut process, &AtomicBool::new(false), Duration::from_millis(200)).unwrap_err();
    assert!(error.to_string().contains("timed out"));
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(
      !Command::new("kill")
        .arg("-0")
        .arg(fs::read_to_string(pid).unwrap().trim())
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success()
    );
  }

  #[tokio::test]
  async fn it_keeps_one_update_running_and_reports_results_in_the_ui() {
    let task = tokio::spawn(std::future::pending());
    let id = task.id();
    let mut ui = super::super::Ui::default();
    ui.update.task = Some(task);
    ui.menu = Some(super::super::menu::Menu::Main(3));
    assert_eq!(
      super::super::menu::handle(&mut ui, ratatui::crossterm::event::KeyCode::Enter),
      Some(false)
    );
    assert_eq!(ui.menu, Some(super::super::menu::Menu::Update));
    let update = &mut ui.update;
    assert_eq!(update.task.as_ref().unwrap().id(), id);
    assert!(!update.poll().await);
    update.task.take().unwrap().abort();
    update.task = Some(tokio::spawn(async { Ok("Already current.".into()) }));
    tokio::task::yield_now().await;
    assert!(update.poll().await);
    assert_eq!(update.message(), "Already current.");
    assert!(update.task.is_none());
    update.task = Some(tokio::spawn(async { Err(Error::Message("checksum mismatch".into())) }));
    tokio::task::yield_now().await;
    assert!(update.poll().await);
    assert!(update.message().contains("checksum mismatch"));
  }

  #[test]
  fn it_selects_only_stable_releases_by_numeric_version() {
    let releases = serde_json::from_value::<Vec<Release>>(serde_json::json!([
      {"tag_name":"0.9.0","draft":false,"prerelease":false,"assets":[]},
      {"tag_name":"0.10.0","draft":false,"prerelease":false,"assets":[]},
      {"tag_name":"1.0.0","draft":true,"prerelease":false,"assets":[]},
      {"tag_name":"2.0.0","draft":false,"prerelease":true,"assets":[]},
      {"tag_name":"3.0.0-rc1","draft":false,"prerelease":false,"assets":[]}
    ]))
    .unwrap();
    assert_eq!(latest(releases).unwrap().tag_name, "0.10.0");
    assert!(version("0.10.0") > version("0.9.0"));
    assert_eq!(target("macos", "aarch64").unwrap(), "aarch64-apple-darwin");
    assert_eq!(target("linux", "x86_64").unwrap(), "x86_64-unknown-linux-gnu");
    assert_eq!(target("windows", "x86_64").unwrap(), "x86_64-pc-windows-gnu");
    assert!(target("linux", "aarch64").is_err());
  }

  #[test]
  fn it_rejects_wrong_missing_and_duplicate_checksums() {
    let directory = tempfile::tempdir().unwrap();
    let archive = directory.path().join("archive");
    fs::write(&archive, b"abc").unwrap();
    let checksum = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    verify(&archive, &format!("{checksum}  archive\n"), "archive").unwrap();
    assert!(verify(&archive, "", "archive").is_err());
    assert!(
      verify(
        &archive,
        &format!("{checksum}  archive\n{checksum}  archive"),
        "archive"
      )
      .is_err()
    );
    fs::write(&archive, b"changed").unwrap();
    assert!(verify(&archive, &format!("{checksum}  archive"), "archive").is_err());
  }

  #[cfg(not(windows))]
  #[test]
  fn it_preserves_the_old_binary_on_failure_and_replaces_it_atomically() {
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("tntop");
    let source = directory.path().join("new");
    fs::write(&destination, b"old").unwrap();
    assert!(install(&source, &destination).is_err());
    assert_eq!(fs::read(&destination).unwrap(), b"old");
    fs::write(&source, b"new").unwrap();
    install(&source, &destination).unwrap();
    assert_eq!(fs::read(&destination).unwrap(), b"new");
  }
}
