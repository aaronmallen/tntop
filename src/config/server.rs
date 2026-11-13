use std::path::{Path, PathBuf};

use getset::Getters;
use serde::{Deserialize, Serialize};
use url::Url;

use super::Error;
use crate::client::Version;

#[derive(Clone, Debug, Deserialize, Getters, Serialize)]
pub struct Server {
  #[serde(default)]
  #[get = "pub"]
  api_version: Version,
  #[get = "pub"]
  ca_cert: Option<PathBuf>,
  #[get = "pub"]
  certificate_sha256: Option<String>,
  #[get = "pub"]
  name: String,
  #[get = "pub"]
  url: Url,
}

impl Server {
  pub fn new(name: String, url: Url) -> Result<Self, Error> {
    let server = Self {
      api_version: Version::default(),
      ca_cert: None,
      certificate_sha256: None,
      name,
      url,
    };
    server.validate()?;
    Ok(server)
  }

  pub(super) fn resolve_paths(&mut self, directory: &Path) {
    if let Some(path) = &mut self.ca_cert
      && path.is_relative()
    {
      *path = directory.join(&*path);
    }
  }

  pub fn validate(&self) -> Result<(), Error> {
    if let Some(pin) = &self.certificate_sha256
      && (pin.len() != 64 || !pin.bytes().all(|byte| byte.is_ascii_hexdigit()) || self.ca_cert.is_some())
    {
      return Err(Error::Invalid(
        "certificate_sha256 must be 64 hex digits and cannot be combined with ca_cert".into(),
      ));
    }
    if self.name.trim().is_empty() || self.name.chars().any(char::is_control) {
      return Err(Error::Invalid(
        "server name must be nonempty and contain no control characters".into(),
      ));
    }
    if self.url.scheme() != "https"
      || self.url.host_str().is_none()
      || !self.url.username().is_empty()
      || self.url.password().is_some()
      || self.url.query().is_some()
      || self.url.fragment().is_some()
      || self.url.path() != "/"
    {
      return Err(Error::Invalid(format!(
        "{}: use an HTTPS server origin, such as https://nas.example.com",
        self.name
      )));
    }
    Ok(())
  }

  pub fn with_ca_cert(mut self, path: PathBuf) -> Self {
    self.ca_cert = Some(path);
    self
  }

  pub fn with_certificate_sha256(mut self, fingerprint: String) -> Self {
    self.ca_cert = None;
    self.certificate_sha256 = Some(fingerprint);
    self
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod server {
    use super::*;

    mod new {
      use super::*;

      #[test]
      fn it_rejects_cleartext_embedded_secrets_and_paths() {
        for url in [
          "http://nas.local",
          "https://user:secret@nas.local",
          "https://nas.local/path",
          "https://nas.local/?key=secret",
        ] {
          assert!(Server::new("nas".into(), url.parse().unwrap()).is_err());
        }
        assert!(Server::new("nas\n".into(), "https://nas.local".parse().unwrap()).is_err());
        assert!(Server::new("nas".into(), "https://nas.local:8443".parse().unwrap()).is_ok());
      }
    }
  }
}
