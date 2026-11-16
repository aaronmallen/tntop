mod env;
mod graph;
mod preset;
mod server;

use std::{
  collections::HashSet,
  io::Write,
  path::{Path, PathBuf},
};

use compote::{Compote, Env, Serialized, Toml};
use getset::{CopyGetters, Getters};
pub use graph::GraphSymbol;
pub use preset::DisabledPresets;
use serde::{Deserialize, Serialize};
pub use server::Server;

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("{0}")]
  Invalid(String),
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[error(transparent)]
  Load(#[from] compote::Error),
  #[error(transparent)]
  Persist(#[from] tempfile::PersistError),
  #[error(transparent)]
  Toml(#[from] toml_edit::TomlError),
  #[error("unable to determine user's ${0} directory")]
  XdgDirNotFound(String),
}

#[derive(Clone, CopyGetters, Debug, Deserialize, Getters, Serialize)]
#[serde(default)]
pub struct Settings {
  #[get_copy = "pub"]
  background_update: bool,
  #[get_copy = "pub"]
  log_level: log::LevelFilter,
  #[get = "pub"]
  servers: Vec<Server>,
  #[get_copy = "pub"]
  save_on_exit: bool,
  #[get_copy = "pub"]
  disable_mouse: bool,
  #[get = "pub"]
  theme: Option<PathBuf>,
  #[get_copy = "pub"]
  theme_background: bool,
  #[get_copy = "pub"]
  truecolor: bool,
  #[get_copy = "pub"]
  force_tty: bool,
  #[get_copy = "pub"]
  show_cpu: bool,
  #[get_copy = "pub"]
  show_mem: bool,
  #[get_copy = "pub"]
  show_net: bool,
  #[get_copy = "pub"]
  show_apps: bool,
  #[get_copy = "pub"]
  cpu_bottom: bool,
  #[get_copy = "pub"]
  apps_left: bool,
  #[get_copy = "pub"]
  mem_below_net: bool,
  presets: String,
  #[get_copy = "pub"]
  disable_presets: DisabledPresets,
  #[get_copy = "pub"]
  rounded_corners: bool,
  #[get_copy = "pub"]
  tty_sync: bool,
  #[get_copy = "pub"]
  vim_keys: bool,
  #[get_copy = "pub"]
  base_10_sizes: bool,
  #[get = "pub"]
  clock_format: String,
  #[get_copy = "pub"]
  graph_symbol: GraphSymbol,
  #[get_copy = "pub"]
  graph_symbol_cpu: GraphSymbol,
  #[get_copy = "pub"]
  graph_symbol_mem: GraphSymbol,
  #[get_copy = "pub"]
  graph_symbol_net: GraphSymbol,
  #[get_copy = "pub"]
  graph_symbol_apps: GraphSymbol,
  #[get_copy = "pub"]
  update_ms: u64,
}

impl Settings {
  pub fn preferences(&self) -> Result<serde_json::Value, Error> {
    serde_json::to_value(self).map_err(|error| Error::Invalid(error.to_string()))
  }

  pub fn with_preference(&self, key: &str, value: serde_json::Value) -> Result<Self, Error> {
    let mut values = self.preferences()?;
    if key == "servers" || values.get(key).is_none() {
      return Err(Error::Invalid(format!("unknown preference: {key}")));
    }
    values[key] = value;
    let candidate: Self = serde_json::from_value(values).map_err(|error| Error::Invalid(error.to_string()))?;
    candidate.with_overrides()
  }

  fn with_overrides(self) -> Result<Self, Error> {
    let candidate: Self = Compote::from(Serialized::defaults(self))
      .merge(Env::prefixed("TNTOP_").split("__").ignore(env::COMPOTE_IGNORE))
      .extract()?;
    candidate.validate()?;
    Ok(candidate)
  }

  pub fn servers_mut(&mut self) -> &mut Vec<Server> {
    &mut self.servers
  }

  pub fn validate(&self) -> Result<(), Error> {
    let epoch = chrono::DateTime::<chrono::Utc>::from(std::time::UNIX_EPOCH);
    std::fmt::write(&mut String::new(), format_args!("{}", epoch.format(&self.clock_format)))
      .map_err(|_| Error::Invalid("clock_format contains an unsupported strftime sequence".into()))?;
    self.presets()?;
    if self.graph_symbol == GraphSymbol::Default {
      return Err(Error::Invalid("graph_symbol must be braille, block or tty".into()));
    }
    if !(100..=86_400_000).contains(&self.update_ms) {
      return Err(Error::Invalid("update_ms must be between 100 and 86400000".into()));
    }
    let mut names = HashSet::new();
    for server in &self.servers {
      server.validate()?;
      if !names.insert(server.name()) {
        return Err(Error::Invalid(format!("duplicate server name: {}", server.name())));
      }
    }
    Ok(())
  }
}

impl Default for Settings {
  fn default() -> Self {
    Self {
      background_update: true,
      log_level: log::LevelFilter::Warn,
      servers: Vec::new(),
      save_on_exit: true,
      disable_mouse: false,
      theme: None,
      theme_background: true,
      truecolor: true,
      force_tty: false,
      show_cpu: true,
      show_mem: true,
      show_net: true,
      show_apps: true,
      cpu_bottom: false,
      apps_left: false,
      mem_below_net: false,
      presets: "cpu:1:default,proc:0:default cpu:0:default,mem:0:default,net:0:default cpu:0:block,net:0:tty".into(),
      disable_presets: DisabledPresets::Off,
      rounded_corners: true,
      tty_sync: true,
      vim_keys: false,
      base_10_sizes: false,
      clock_format: "%H:%M:%S UTC".into(),
      graph_symbol: GraphSymbol::Braille,
      graph_symbol_cpu: GraphSymbol::Default,
      graph_symbol_mem: GraphSymbol::Default,
      graph_symbol_net: GraphSymbol::Default,
      graph_symbol_apps: GraphSymbol::Default,
      update_ms: 2000,
    }
  }
}

pub fn save_preferences(path: &Path, before: &Settings, after: &Settings) -> Result<(), Error> {
  after.validate()?;
  let before = serde_json::to_value(before).map_err(|error| Error::Invalid(error.to_string()))?;
  let after = serde_json::to_value(after).map_err(|error| Error::Invalid(error.to_string()))?;
  let source = match std::fs::read_to_string(path) {
    Ok(source) => source,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
    Err(error) => return Err(error.into()),
  };
  let mut document = source.parse::<toml_edit::DocumentMut>()?;
  let mut changed = false;
  for (key, value) in after.as_object().unwrap() {
    if key == "servers"
      || before.get(key) == Some(value)
      || std::env::var_os(format!("TNTOP_{}", key.to_uppercase())).is_some()
    {
      continue;
    }
    let replacement = match value {
      serde_json::Value::Null => None,
      serde_json::Value::Bool(value) => Some(toml_edit::Value::from(*value)),
      serde_json::Value::String(value) => Some(toml_edit::Value::from(value.as_str())),
      serde_json::Value::Number(value) => Some(toml_edit::Value::from(
        value
          .as_i64()
          .ok_or_else(|| Error::Invalid(format!("invalid preference: {key}")))?,
      )),
      _ => return Err(Error::Invalid(format!("unsupported preference: {key}"))),
    };
    if let Some(mut value) = replacement {
      if let Some(old) = document.get(key).and_then(toml_edit::Item::as_value) {
        *value.decor_mut() = old.decor().clone();
      }
      document[key] = toml_edit::Item::Value(value);
    } else {
      document.remove(key);
    }
    changed = true;
  }
  if changed {
    save(path, &document)?;
  }
  Ok(())
}

pub fn add_server(path: &Path, server: &Server) -> Result<(), Error> {
  server.validate()?;
  let source = match std::fs::read_to_string(path) {
    Ok(source) => source,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
    Err(error) => return Err(error.into()),
  };
  let mut document = source.parse::<toml_edit::DocumentMut>()?;
  if document.get("servers").is_none() {
    document["servers"] = toml_edit::Item::ArrayOfTables(toml_edit::ArrayOfTables::new());
  }
  let servers = document["servers"]
    .as_array_of_tables_mut()
    .ok_or_else(|| Error::Invalid("servers must use [[servers]] tables".into()))?;
  if servers
    .iter()
    .any(|entry| entry.get("name").and_then(toml_edit::Item::as_str) == Some(server.name()))
  {
    return Err(Error::Invalid(format!("server already exists: {}", server.name())));
  }
  let mut table = toml_edit::Table::new();
  table["name"] = toml_edit::value(server.name());
  table["url"] = toml_edit::value(server.url().as_str());
  table["api_version"] = toml_edit::value(server.api_version().as_str());
  if let Some(path) = server.ca_cert() {
    table["ca_cert"] = toml_edit::value(
      path
        .to_str()
        .ok_or_else(|| Error::Invalid("CA certificate path must be UTF-8".into()))?,
    );
  }
  servers.push(table);
  save(path, &document)
}

pub fn default_path() -> Result<PathBuf, Error> {
  if let Ok(path) = env::CONFIG_PATH.value() {
    log::debug!("{} is set to {}", env::CONFIG_PATH.name(), path.display());
    if path.is_absolute() {
      return Ok(path);
    }
    log::warn!(
      "{} is not an absolute path, using default path",
      env::CONFIG_PATH.name()
    );
  }

  dir_spec::config_home()
    .map(|path| path.join("tntop/config.toml"))
    .ok_or_else(|| Error::XdgDirNotFound("config".into()))
}

pub fn load(path: &Path) -> Result<Settings, Error> {
  log::trace!("Loading configuration from {}", path.display());

  let mut settings: Settings = Compote::from(Serialized::defaults(Settings::default()))
    .merge(Toml::path(path))
    .merge(Env::prefixed("TNTOP_").split("__").ignore(env::COMPOTE_IGNORE))
    .extract()?;
  settings.validate()?;
  let directory = std::path::absolute(path)?.parent().unwrap_or(Path::new(".")).to_owned();
  for server in &mut settings.servers {
    server.resolve_paths(&directory);
  }
  Ok(settings)
}

pub fn pin_certificate(path: &Path, server: &Server, fingerprint: &str) -> Result<(), Error> {
  server.clone().with_certificate_sha256(fingerprint.into()).validate()?;
  let mut document = std::fs::read_to_string(path)?.parse::<toml_edit::DocumentMut>()?;
  let entry = document
    .get_mut("servers")
    .and_then(toml_edit::Item::as_array_of_tables_mut)
    .and_then(|servers| {
      servers.iter_mut().find(|entry| {
        entry.get("name").and_then(toml_edit::Item::as_str) == Some(server.name())
          && entry
            .get("url")
            .and_then(toml_edit::Item::as_str)
            .and_then(|url| url.parse::<url::Url>().ok())
            .as_ref()
            == Some(server.url())
      })
    })
    .ok_or_else(|| Error::Invalid("server changed or is missing from the config file".into()))?;
  let directory = std::path::absolute(path)?.parent().unwrap_or(Path::new(".")).to_owned();
  let ca_cert = entry
    .get("ca_cert")
    .and_then(toml_edit::Item::as_str)
    .map(|path| directory.join(path));
  if entry.get("certificate_sha256").and_then(toml_edit::Item::as_str) != server.certificate_sha256().as_deref()
    || &ca_cert != server.ca_cert()
  {
    return Err(Error::Invalid("server trust settings changed; restart tntop".into()));
  }
  entry["certificate_sha256"] = toml_edit::value(fingerprint);
  entry.remove("ca_cert");
  save(path, &document)
}

pub fn remove_server(path: &Path, server: &Server) -> Result<(), Error> {
  let mut document = std::fs::read_to_string(path)?.parse::<toml_edit::DocumentMut>()?;
  let servers = document
    .get_mut("servers")
    .and_then(toml_edit::Item::as_array_of_tables_mut)
    .ok_or_else(|| Error::Invalid("servers must use [[servers]] tables".into()))?;
  let index = servers
    .iter()
    .position(|entry| {
      entry.get("name").and_then(toml_edit::Item::as_str) == Some(server.name())
        && entry
          .get("url")
          .and_then(toml_edit::Item::as_str)
          .and_then(|url| url.parse::<url::Url>().ok())
          .as_ref()
          == Some(server.url())
    })
    .ok_or_else(|| Error::Invalid("server changed or is missing from the config file".into()))?;
  servers.remove(index);
  save(path, &document)
}
fn save(path: &Path, document: &toml_edit::DocumentMut) -> Result<(), Error> {
  let parent = path
    .parent()
    .filter(|parent| !parent.as_os_str().is_empty())
    .unwrap_or(Path::new("."));
  std::fs::create_dir_all(parent)?;
  let mut file = tempfile::NamedTempFile::new_in(parent)?;
  file.write_all(document.to_string().as_bytes())?;
  file.as_file().sync_all()?;
  file.persist(path)?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  mod save_preferences {
    use super::*;

    #[test]
    fn it_keeps_environment_overrides_out_of_the_file() {
      if std::env::var_os("_TNTOP_PREFS_CHILD").is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
          .args([
            "--exact",
            "config::tests::save_preferences::it_keeps_environment_overrides_out_of_the_file",
          ])
          .env("_TNTOP_PREFS_CHILD", "1")
          .env("TNTOP_UPDATE_MS", "700")
          .status()
          .unwrap();
        assert!(status.success());
        return;
      }
      let directory = tempfile::tempdir().unwrap();
      let path = directory.path().join("config.toml");
      std::fs::write(&path, "update_ms = 500\n").unwrap();
      let before = load(&path).unwrap();
      assert_eq!(before.update_ms(), 700);
      let mut after = before.clone();
      after.update_ms = 900;
      super::super::save_preferences(&path, &before, &after).unwrap();
      assert_eq!(std::fs::read_to_string(&path).unwrap(), "update_ms = 500\n");
    }

    #[test]
    fn it_changes_only_preferences_and_keeps_comments_unknown_values_and_trust() {
      let directory = tempfile::tempdir().unwrap();
      let path = directory.path().join("config.toml");
      let source = format!(
        "# keep\nupdate_ms = 500 # cadence\nunknown = 'keep'\n[[servers]]\nname='nas'\nurl='https://nas.local'\ncertificate_sha256='{}'\n",
        "ab".repeat(32)
      );
      std::fs::write(&path, &source).unwrap();
      let before = load(&path).unwrap();
      let mut after = before.clone();
      after.update_ms = 1000;
      after.servers.clear();
      super::super::save_preferences(&path, &before, &after).unwrap();
      let saved = std::fs::read_to_string(&path).unwrap();
      assert!(saved.starts_with("# keep\nupdate_ms = 1000 # cadence\nunknown = 'keep'"));
      assert_eq!(
        load(&path).unwrap().servers()[0].certificate_sha256(),
        &Some("ab".repeat(32))
      );
      assert_eq!(load(&path).unwrap().update_ms(), 1000);
    }

    #[test]
    fn it_keeps_the_file_on_invalid_preferences_or_write_failure() {
      let directory = tempfile::tempdir().unwrap();
      let path = directory.path().join("config.toml");
      std::fs::write(&path, "update_ms = 500\n").unwrap();
      let before = load(&path).unwrap();
      let mut after = before.clone();
      after.update_ms = 0;
      assert!(super::super::save_preferences(&path, &before, &after).is_err());
      assert_eq!(std::fs::read_to_string(&path).unwrap(), "update_ms = 500\n");
      after.update_ms = 1000;
      assert!(super::super::save_preferences(&path.join("blocked"), &before, &after).is_err());
      assert_eq!(std::fs::read_to_string(&path).unwrap(), "update_ms = 500\n");
    }
  }

  mod add_server {
    #[test]
    fn it_saves_and_reloads_http_servers() {
      let directory = tempfile::tempdir().unwrap();
      let path = directory.path().join("config.toml");
      let server = super::Server::new("nas".into(), "http://nas.local:8080".parse().unwrap()).unwrap();
      super::add_server(&path, &server).unwrap();
      let saved = super::load(&path).unwrap();
      assert_eq!(saved.servers()[0].url(), server.url());
      assert!(saved.servers()[0].certificate_sha256().is_none());
    }

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_preserves_settings_and_comments_and_rejects_duplicates() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      std::fs::write(&path, "# keep this\nupdate_ms = 500\n").unwrap();
      let server = Server::new("nas".into(), "https://nas.example.com".parse().unwrap())
        .unwrap()
        .with_ca_cert(PathBuf::from("certs/nas.pem"));
      super::super::add_server(&path, &server).unwrap();
      let source = std::fs::read_to_string(&path).unwrap();
      assert!(source.starts_with("# keep this\nupdate_ms = 500"));
      let settings = load(&path).unwrap();
      assert_eq!(settings.servers().len(), 1);
      assert_eq!(settings.servers()[0].name(), "nas");
      assert_eq!(settings.servers()[0].ca_cert(), &Some(dir.path().join("certs/nas.pem")));
      assert!(source.contains("ca_cert = \"certs/nas.pem\""));
      assert_eq!(settings.update_ms(), 500);
      assert!(super::super::add_server(&path, &server).is_err());
      assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    }
  }

  mod load {
    use super::*;

    #[test]
    fn it_allows_first_run_without_a_config_file() {
      let dir = tempfile::tempdir().unwrap();
      assert!(load(&dir.path().join("missing.toml")).unwrap().servers().is_empty());
    }

    #[test]
    fn it_rejects_duplicate_servers_and_zero_refresh() {
      for source in [
        "update_ms = 0",
        "[[servers]]\nname='nas'\nurl='https://nas.example.com'\n[[servers]]\nname='nas'\nurl='https://other.example.com'",
      ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, source).unwrap();
        assert!(load(&path).is_err());
      }
    }
  }

  mod pin_certificate {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_persists_trust_without_changing_other_settings_or_replacing_a_pin() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      std::fs::write(&path, "# keep\nupdate_ms = 500\n").unwrap();
      let server = Server::new("nas".into(), "https://nas.local".parse().unwrap()).unwrap();
      add_server(&path, &server).unwrap();
      let pin = "ab".repeat(32);
      pin_certificate(&path, &server, &pin).unwrap();
      let settings = load(&path).unwrap();
      assert_eq!(
        settings.servers()[0].certificate_sha256().as_deref(),
        Some(pin.as_str())
      );
      assert_eq!(settings.update_ms(), 500);
      assert!(std::fs::read_to_string(&path).unwrap().starts_with("# keep"));
      assert!(pin_certificate(&path, &server, &"cd".repeat(32)).is_err());
      assert_eq!(load(&path).unwrap().servers()[0].certificate_sha256(), &Some(pin));
    }

    #[test]
    fn it_replaces_only_the_trust_that_was_presented_for_approval() {
      let directory = tempfile::tempdir().unwrap();
      let path = directory.path().join("config.toml");
      let server = Server::new("nas".into(), "https://nas.local".parse().unwrap()).unwrap();
      add_server(&path, &server).unwrap();
      pin_certificate(&path, &server, &"ab".repeat(32)).unwrap();
      let approved = load(&path).unwrap().servers()[0].clone();
      assert!(pin_certificate(&path, &approved, "bad fingerprint").is_err());
      pin_certificate(&path, &approved, &"cd".repeat(32)).unwrap();
      assert!(pin_certificate(&path, &approved, &"ef".repeat(32)).is_err());
      assert_eq!(
        load(&path).unwrap().servers()[0].certificate_sha256(),
        &Some("cd".repeat(32))
      );
    }

    #[test]
    fn it_switches_an_approved_ca_server_to_pinning() {
      let directory = tempfile::tempdir().unwrap();
      let path = directory.path().join("config.toml");
      let server = Server::new("nas".into(), "https://nas.local".parse().unwrap())
        .unwrap()
        .with_ca_cert("certs/nas.pem".into());
      add_server(&path, &server).unwrap();
      let server = load(&path).unwrap().servers()[0].clone();
      pin_certificate(&path, &server, &"ab".repeat(32)).unwrap();
      let settings = load(&path).unwrap();
      assert!(settings.servers()[0].ca_cert().is_none());
      assert_eq!(settings.servers()[0].certificate_sha256(), &Some("ab".repeat(32)));
    }
  }

  mod remove_server {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_removes_only_the_selected_entry_and_allows_removing_the_last_server() {
      let directory = tempfile::tempdir().unwrap();
      let path = directory.path().join("config.toml");
      std::fs::write(&path, "# keep\nupdate_ms = 500\n").unwrap();
      let first = Server::new("first".into(), "https://nas.local".parse().unwrap()).unwrap();
      let second = Server::new("second".into(), first.url().clone()).unwrap();
      add_server(&path, &first).unwrap();
      add_server(&path, &second).unwrap();
      let changed = Server::new("first".into(), "https://other.local".parse().unwrap()).unwrap();
      assert!(remove_server(&path, &changed).is_err());
      remove_server(&path, &first).unwrap();
      let settings = load(&path).unwrap();
      assert_eq!(settings.servers().len(), 1);
      assert_eq!(settings.servers()[0].name(), "second");
      remove_server(&path, &second).unwrap();
      assert!(load(&path).unwrap().servers().is_empty());
      assert!(
        std::fs::read_to_string(&path)
          .unwrap()
          .starts_with("# keep\nupdate_ms = 500")
      );
    }
  }
}
