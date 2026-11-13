use std::{
  path::Path,
  sync::{Arc, Mutex},
  time::Duration,
};

use rustls::{
  ClientConfig, RootCertStore,
  client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
  pki_types::{CertificateDer, ServerName, UnixTime, pem::PemObject},
};
use tokio_tungstenite::Connector;

use super::Error;

#[derive(Debug)]
struct Verifier {
  observed: Mutex<Option<String>>,
  pin: Option<String>,
}

impl ServerCertVerifier for Verifier {
  fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
    rustls::crypto::ring::default_provider()
      .signature_verification_algorithms
      .supported_schemes()
  }

  fn verify_server_cert(
    &self,
    certificate: &CertificateDer<'_>,
    _intermediates: &[CertificateDer<'_>],
    _name: &ServerName<'_>,
    _ocsp: &[u8],
    _now: UnixTime,
  ) -> Result<ServerCertVerified, rustls::Error> {
    let actual = fingerprint(certificate);
    match &self.pin {
      Some(expected) if expected.eq_ignore_ascii_case(&actual) => Ok(ServerCertVerified::assertion()),
      Some(expected) => Err(rustls::Error::General(format!(
        "server certificate changed: expected SHA-256 {expected}, received {actual}; verify the change before replacing the saved fingerprint"
      ))),
      None => {
        *self.observed.lock().unwrap() = Some(actual);
        Err(rustls::Error::General("certificate trust required".into()))
      }
    }
  }

  fn verify_tls12_signature(
    &self,
    message: &[u8],
    certificate: &CertificateDer<'_>,
    signature: &rustls::DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, rustls::Error> {
    rustls::crypto::verify_tls12_signature(
      message,
      certificate,
      signature,
      &rustls::crypto::ring::default_provider().signature_verification_algorithms,
    )
  }

  fn verify_tls13_signature(
    &self,
    message: &[u8],
    certificate: &CertificateDer<'_>,
    signature: &rustls::DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, rustls::Error> {
    rustls::crypto::verify_tls13_signature(
      message,
      certificate,
      signature,
      &rustls::crypto::ring::default_provider().signature_verification_algorithms,
    )
  }
}

pub(super) fn connector(path: Option<&Path>, pin: Option<&str>) -> Result<Option<Connector>, Error> {
  if let Some(pin) = pin {
    return Ok(Some(custom_connector(Arc::new(Verifier {
      pin: Some(pin.to_owned()),
      observed: Mutex::new(None),
    }))));
  }
  let Some(path) = path else { return Ok(None) };
  let certificates = CertificateDer::pem_file_iter(path)
    .and_then(|certificates| certificates.collect::<Result<Vec<_>, _>>())
    .map_err(|error| Error::CaCertificate {
      path: path.to_owned(),
      reason: error.to_string(),
    })?;
  if certificates.is_empty() {
    return Err(Error::CaCertificate {
      path: path.to_owned(),
      reason: "file contains no PEM certificates".into(),
    });
  }
  let mut roots = RootCertStore::empty();
  roots.add_parsable_certificates(rustls_native_certs::load_native_certs().certs);
  for certificate in certificates {
    roots.add(certificate).map_err(|error| Error::CaCertificate {
      path: path.to_owned(),
      reason: error.to_string(),
    })?;
  }
  let config = ClientConfig::builder()
    .with_root_certificates(roots)
    .with_no_client_auth();
  Ok(Some(Connector::Rustls(Arc::new(config))))
}

pub(crate) async fn discover(server: &crate::config::Server) -> Result<String, Error> {
  let verifier = Arc::new(Verifier {
    pin: None,
    observed: Mutex::new(None),
  });
  let mut url = server.url().clone();
  url
    .set_scheme("wss")
    .map_err(|_| Error::Protocol("invalid server URL".into()))?;
  let result = tokio::time::timeout(
    Duration::from_secs(15),
    tokio_tungstenite::connect_async_tls_with_config(
      url.as_str(),
      None,
      false,
      Some(custom_connector(verifier.clone())),
    ),
  )
  .await?;
  if let Some(pin) = verifier.observed.lock().unwrap().take() {
    return Ok(pin);
  }
  result?;
  Err(Error::Protocol("server did not present a certificate".into()))
}

fn custom_connector(verifier: Arc<Verifier>) -> Connector {
  Connector::Rustls(Arc::new(
    ClientConfig::builder()
      .dangerous()
      .with_custom_certificate_verifier(verifier)
      .with_no_client_auth(),
  ))
}

fn fingerprint(certificate: &CertificateDer<'_>) -> String {
  ring::digest::digest(&ring::digest::SHA256, certificate.as_ref())
    .as_ref()
    .iter()
    .map(|byte| format!("{byte:02x}"))
    .collect()
}

#[cfg(test)]
mod tests {
  use futures_util::{SinkExt, StreamExt};
  use rustls::{ServerConfig, pki_types::PrivatePkcs8KeyDer};
  use serde_json::{Value, json};
  use tokio::net::TcpListener;
  use tokio_tungstenite::tungstenite::Message;

  use super::*;
  use crate::{client::Client, config::Server};

  mod connector {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_rejects_missing_empty_and_malformed_certificate_files() {
      let directory = tempfile::tempdir().unwrap();
      let path = directory.path().join("nas.pem");
      assert!(matches!(
        super::super::connector(Some(&path), None),
        Err(Error::CaCertificate { .. })
      ));
      for source in [
        "",
        "not a certificate",
        "-----BEGIN CERTIFICATE-----\ninvalid\n-----END CERTIFICATE-----",
        "-----BEGIN CERTIFICATE-----\nAA==\n-----END CERTIFICATE-----",
      ] {
        std::fs::write(&path, source).unwrap();
        assert!(matches!(
          super::super::connector(Some(&path), None),
          Err(Error::CaCertificate { .. })
        ));
      }
    }

    #[tokio::test]
    async fn it_trusts_only_the_configured_certificate_and_checks_the_hostname() {
      for (mode, hostname, expected) in [
        ("system", "127.0.0.1", false),
        ("ca", "127.0.0.1", true),
        ("ca", "wrong.example.com", false),
        ("pin", "localhost", true),
        ("changed", "localhost", false),
        ("discover", "localhost", false),
      ] {
        let certificate = rcgen::generate_simple_self_signed(vec![hostname.into()]).unwrap();
        let pin = fingerprint(certificate.cert.der());
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nas.pem");
        std::fs::write(&path, certificate.cert.pem()).unwrap();
        let config = ServerConfig::builder()
          .with_no_client_auth()
          .with_single_cert(
            vec![certificate.cert.der().clone()],
            PrivatePkcs8KeyDer::from(certificate.signing_key.serialize_der()).into(),
          )
          .unwrap();
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let peer = tokio::spawn(async move {
          let (stream, _) = listener.accept().await.unwrap();
          let Ok(stream) = acceptor.accept(stream).await else {
            return false;
          };
          let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
          let request: Value = serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
          assert_eq!(request["method"], "auth.login_with_api_key");
          assert_eq!(request["params"], json!(["test-api-key"]));
          socket
            .send(Message::Text(
              json!({"jsonrpc":"2.0","id":request["id"],"result":true})
                .to_string()
                .into(),
            ))
            .await
            .unwrap();
          true
        });
        let mut server = Server::new("nas".into(), format!("https://{address}").parse().unwrap()).unwrap();
        if mode == "ca" {
          server = server.with_ca_cert(path);
        } else if mode == "pin" {
          server = server.with_certificate_sha256(pin.clone());
        } else if mode == "changed" {
          server = server.with_certificate_sha256("0".repeat(64));
        }
        if mode == "discover" {
          assert_eq!(discover(&server).await.unwrap(), pin);
          assert!(!peer.await.unwrap(), "discovery must stop before application traffic");
          continue;
        }
        let result = Client::connect(&server, "test-api-key").await;
        assert_eq!(result.is_ok(), expected, "mode={mode}, hostname={hostname}");
        if mode == "changed" {
          assert!(result.err().unwrap().to_string().contains("server certificate changed"));
        }
        assert_eq!(peer.await.unwrap(), expected);
      }
    }
  }
}
