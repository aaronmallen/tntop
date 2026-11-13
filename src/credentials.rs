use zeroize::Zeroizing;

use crate::config::Server;

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("API key cannot be empty")]
  Empty,
  #[error(transparent)]
  Join(#[from] tokio::task::JoinError),
  #[error("keyring: {0}")]
  Keyring(#[from] keyring::Error),
}

pub async fn delete(server: &Server) -> Result<(), Error> {
  let account = server.url().as_str().to_owned();
  tokio::task::spawn_blocking(move || {
    match keyring::Entry::new("dev.aaronmallen.tntop", &account)?.delete_credential() {
      Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
      Err(error) => Err(error),
    }
  })
  .await??;
  Ok(())
}

pub async fn load(server: &Server) -> Result<Option<Zeroizing<String>>, Error> {
  let account = server.url().as_str().to_owned();
  Ok(
    tokio::task::spawn_blocking(
      move || match keyring::Entry::new("dev.aaronmallen.tntop", &account)?.get_password() {
        Ok(key) => Ok(Some(Zeroizing::new(key))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error),
      },
    )
    .await??,
  )
}

pub async fn save(server: &Server, key: Zeroizing<String>) -> Result<(), Error> {
  if key.trim().is_empty() {
    return Err(Error::Empty);
  }
  let account = server.url().as_str().to_owned();
  tokio::task::spawn_blocking(move || keyring::Entry::new("dev.aaronmallen.tntop", &account)?.set_password(&key))
    .await??;
  Ok(())
}
