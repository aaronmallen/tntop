mod model;
pub(crate) mod tls;
mod v25_10;

use std::{
  collections::{BTreeMap, VecDeque},
  sync::Arc,
  time::{Duration, Instant},
};

use futures_util::{SinkExt, StreamExt};
use getset::Getters;
pub use model::{AppStats, Application, Metrics, NetworkTotals, SystemInfo};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{
  net::TcpStream,
  sync::watch,
  time::{MissedTickBehavior, timeout},
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, tungstenite::Message};
use zeroize::Zeroizing;

use crate::config::Server;

pub struct Client {
  id: u64,
  pending: VecDeque<Value>,
  socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl Client {
  pub async fn connect(server: &Server, key: &str) -> Result<Self, Error> {
    server.validate().map_err(|error| Error::Protocol(error.to_string()))?;
    let secure = server.url().scheme() == "https";
    let mut url = server.url().clone();
    url
      .set_scheme(if secure { "wss" } else { "ws" })
      .map_err(|_| Error::Protocol("invalid server URL".into()))?;
    url.set_path(&format!("/api/{}", server.api_version().as_str()));
    let connector = if secure {
      tls::connector(server.ca_cert().as_deref(), server.certificate_sha256().as_deref())?
    } else {
      None
    };
    let (socket, _) = timeout(
      Duration::from_secs(15),
      tokio_tungstenite::connect_async_tls_with_config(url.as_str(), None, false, connector),
    )
    .await??;
    let mut client = Self {
      id: 0,
      pending: VecDeque::new(),
      socket,
    };
    if client.call("auth.login_with_api_key", json!([key])).await? != Value::Bool(true) {
      return Err(Error::Authentication);
    }
    Ok(client)
  }

  async fn call(&mut self, method: &str, params: Value) -> Result<Value, Error> {
    timeout(Duration::from_secs(15), async {
      self.id += 1;
      let id = self.id;
      self
        .socket
        .send(Message::Text(
          json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
            .to_string()
            .into(),
        ))
        .await?;
      loop {
        let message = self.receive().await?;
        if message["id"].as_u64() == Some(id) {
          if let Some(error) = message.get("error") {
            return Err(Error::Rpc {
              method: method.into(),
              code: error["code"].as_i64().unwrap_or_default(),
            });
          }
          return message
            .get("result")
            .cloned()
            .ok_or_else(|| Error::Protocol("missing RPC result".into()));
        }
        if self.pending.len() == 64 {
          self.pending.pop_front();
        }
        self.pending.push_back(message);
      }
    })
    .await?
  }

  async fn next(&mut self) -> Result<Value, Error> {
    if let Some(message) = self.pending.pop_front() {
      return Ok(message);
    }
    self.receive().await
  }

  async fn receive(&mut self) -> Result<Value, Error> {
    loop {
      match self.socket.next().await.ok_or(Error::Closed)?? {
        Message::Text(text) => return Ok(serde_json::from_str(&text)?),
        Message::Ping(payload) => self.socket.send(Message::Pong(payload)).await?,
        Message::Close(_) => return Err(Error::Closed),
        _ => {}
      }
    }
  }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("API key rejected; replace this server's key in your system keyring")]
  Authentication,
  #[error("cannot load CA certificate {}: {reason}", path.display())]
  CaCertificate { path: std::path::PathBuf, reason: String },
  #[error("connection closed")]
  Closed,
  #[error("invalid API response: {0}")]
  Json(#[from] serde_json::Error),
  #[error("{0}")]
  Protocol(String),
  #[error("{method} failed (RPC {code}); check the API key's roles and server API version")]
  Rpc { code: i64, method: String },
  #[error("server did not respond in time")]
  Timeout(#[from] tokio::time::error::Elapsed),
  #[error("WebSocket connection: {0}")]
  WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
}

#[derive(Clone, Default, Getters)]
#[get = "pub"]
pub struct State {
  app_history: VecDeque<Arc<BTreeMap<String, AppStats>>>,
  app_stats_error: Option<String>,
  app_stats_updated_at: Option<Instant>,
  apps: Option<Vec<Application>>,
  apps_error: Option<String>,
  apps_updated_at: Option<Instant>,
  error: Option<String>,
  history: VecDeque<Arc<Metrics>>,
  network_totals: BTreeMap<String, NetworkTotals>,
  system_error: Option<String>,
  system_info: Option<SystemInfo>,
  system_updated_at: Option<Instant>,
  updated_at: Option<Instant>,
  version: String,
}

impl State {
  fn record_metrics(&mut self, metrics: Metrics, now: Instant, elapsed: Option<Duration>) {
    for (name, interface) in metrics.interfaces() {
      let continuous = self
        .history
        .back()
        .is_some_and(|previous| previous.interfaces().contains_key(name));
      self
        .network_totals
        .entry(name.clone())
        .or_default()
        .record(interface, elapsed.filter(|_| continuous));
    }
    if self.history.len() == 600 {
      self.history.pop_front();
    }
    self.history.push_back(Arc::new(metrics));
    self.updated_at = Some(now);
    self.error = None;
  }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
pub enum Version {
  #[default]
  #[serde(rename = "v25.10.0")]
  V25_10,
}

impl Version {
  pub fn as_str(&self) -> &'static str {
    match self {
      Self::V25_10 => "v25.10.0",
    }
  }

  fn app_stats(&self, value: Value) -> Result<BTreeMap<String, AppStats>, Error> {
    match self {
      Self::V25_10 => v25_10::app_stats(value),
    }
  }

  fn apps(&self, value: Value) -> Result<Vec<Application>, Error> {
    match self {
      Self::V25_10 => Ok(serde_json::from_value(value)?),
    }
  }

  fn metrics(&self, value: Value) -> Result<Metrics, Error> {
    match self {
      Self::V25_10 => v25_10::metrics(value),
    }
  }

  fn system_info(&self, value: Value) -> Result<SystemInfo, Error> {
    match self {
      Self::V25_10 => Ok(serde_json::from_value(value)?),
    }
  }
}

pub fn watch(
  server: Server,
  key: Zeroizing<String>,
  update_ms: u64,
) -> (watch::Receiver<State>, watch::Sender<u64>, tokio::task::JoinHandle<()>) {
  let (sender, receiver) = watch::channel(State::default());
  let (interval, mut updates) = watch::channel(update_ms.max(2000));
  let task = tokio::spawn(async move {
    let mut state = State::default();
    let mut delay = 2;
    loop {
      let update_ms = *updates.borrow_and_update();
      log::debug!("starting collection at {update_ms} ms (API minimum 2000 ms)");
      let result = tokio::select! {
        result = monitor(&server, &key, update_ms, &sender, &mut state) => result,
        changed = updates.changed() => {
          if changed.is_err() { break; }
          delay = 2;
          continue;
        }
      };
      if sender.is_closed() {
        break;
      }
      if let Err(error) = result {
        log::warn!("collection stopped: {error}");
        state.error = Some(error.to_string());
        sender.send_replace(state.clone());
        if matches!(error, Error::Authentication) {
          break;
        }
      }
      tokio::select! {
        _ = sender.closed() => break,
        _ = tokio::time::sleep(Duration::from_secs(delay)) => {},
        changed = updates.changed() => {
          if changed.is_err() { break; }
          delay = 2;
          continue;
        }
      }
      delay = (delay * 2).min(30);
    }
  });
  (receiver, interval, task)
}

async fn monitor(
  server: &Server,
  key: &str,
  update_ms: u64,
  sender: &watch::Sender<State>,
  state: &mut State,
) -> Result<(), Error> {
  let mut client = Client::connect(server, key).await?;
  state.version = client
    .call("system.version", json!([]))
    .await?
    .as_str()
    .ok_or_else(|| Error::Protocol("missing system version".into()))?
    .to_owned();
  let interval = update_ms.div_ceil(1000).max(2);
  let collection = format!("reporting.realtime:{{\"interval\":{interval}}}");
  client.call("core.subscribe", json!([collection])).await?;
  let app_collection = format!("app.stats:{{\"interval\":{interval}}}");
  let mut app_subscribed = false;
  let mut retry_subscription = tokio::time::interval(Duration::from_secs(30));
  retry_subscription.set_missed_tick_behavior(MissedTickBehavior::Skip);
  let mut refresh = tokio::time::interval(Duration::from_millis(update_ms.max(2000)));
  refresh.set_missed_tick_behavior(MissedTickBehavior::Skip);
  let mut last_sample = Instant::now();
  let mut previous_sample = None;
  loop {
    tokio::select! {
      _ = sender.closed() => return Ok(()),
      _ = retry_subscription.tick() => {
        if !app_subscribed {
          match client.call("core.subscribe", json!([app_collection])).await {
            Ok(_) => { app_subscribed = true; state.app_stats_error = None; }
            Err(error) => state.app_stats_error = Some(error.to_string()),
          }
        }
        sender.send_replace(state.clone());
      }
      _ = refresh.tick() => {
        match client.call("system.info", json!([])).await.and_then(|value| server.api_version().system_info(value)) {
          Ok(info) => {
            state.system_info = Some(info);
            state.system_error = None;
            state.system_updated_at = Some(Instant::now());
          }
          Err(error) => state.system_error = Some(error.to_string()),
        }
        match client.call("app.query", json!([])).await.and_then(|value| server.api_version().apps(value)) {
          Ok(apps) => {
            state.apps = Some(apps);
            state.apps_error = None;
            state.apps_updated_at = Some(Instant::now());
          }
          Err(error) => state.apps_error = Some(error.to_string()),
        }
        sender.send_replace(state.clone());
      }
      message = client.next() => {
        let message = message?;
        let name = message["params"]["collection"].as_str().unwrap_or("");
        if message["method"] == "notify_unsubscribed" {
          if name == app_collection || name == "app.stats" {
            app_subscribed = false;
            state.app_stats_error = Some("app stats subscription ended; retrying".into());
            sender.send_replace(state.clone());
          } else {
            return Err(Error::Protocol("server ended the metrics subscription; check REPORTING_READ permission".into()));
          }
        }
        if message["method"] == "collection_update"
          && (name == collection || name == "reporting.realtime")
        {
          let metrics = server.api_version().metrics(message["params"]["fields"].clone())?;
          last_sample = Instant::now();
          state.record_metrics(metrics, last_sample, previous_sample.map(|previous| last_sample.duration_since(previous)));
          previous_sample = Some(last_sample);
          sender.send_replace(state.clone());
        } else if message["method"] == "collection_update" && (name == app_collection || name == "app.stats") {
          match server.api_version().app_stats(message["params"]["fields"].clone()) {
            Ok(stats) => {
              if state.app_history.len() == 120 { state.app_history.pop_front(); }
              state.app_history.push_back(Arc::new(stats));
              state.app_stats_updated_at = Some(Instant::now());
              state.app_stats_error = None;
            }
            Err(error) => state.app_stats_error = Some(error.to_string()),
          }
          sender.send_replace(state.clone());
        }
      }
      _ = tokio::time::sleep_until(tokio::time::Instant::from_std(last_sample + Duration::from_secs((interval * 3).max(20)))) => {
        return Err(Error::Protocol("metrics feed stalled; reconnecting".into()));
      }
    }
  }
}

#[cfg(test)]
pub(crate) mod fixtures;

#[cfg(test)]
mod tests {
  use rustls::{ServerConfig, pki_types::PrivatePkcs8KeyDer};
  use tokio::net::TcpListener;

  use super::*;

  async fn server(
    authenticated: bool,
    details_allowed: bool,
    intervals: Vec<u64>,
  ) -> (Server, tokio::task::JoinHandle<()>) {
    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let pin: String = ring::digest::digest(&ring::digest::SHA256, certificate.cert.der())
      .as_ref()
      .iter()
      .map(|byte| format!("{byte:02x}"))
      .collect();
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
    let task = tokio::spawn(async move {
      for interval in intervals {
        let (stream, _) = listener.accept().await.unwrap();
        let stream = acceptor.accept(stream).await.unwrap();
        let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
        while let Some(Ok(message)) = socket.next().await {
          if !message.is_text() {
            continue;
          }
          let request: Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
          let method = request["method"].as_str().unwrap();
          let result = match method {
            "auth.login_with_api_key" => {
              pretty_assertions::assert_eq!(request["params"], json!(["test-api-key"]));
              json!(authenticated)
            }
            "system.version" => json!("TrueNAS-25.10.8"),
            "core.subscribe" => json!(true),
            "system.info" => json!({"model":"Test CPU","loadavg":[1.0,2.0,3.0],"uptime_seconds":120}),
            "app.query" => {
              json!([{"name":"postgres","state":"RUNNING","image_updates_available":false,"upgrade_available":true}])
            }
            _ => panic!("unexpected method {method}"),
          };
          let denied = !details_allowed
            && (method == "system.info"
              || method == "app.query"
              || (method == "core.subscribe" && request["params"][0].as_str().unwrap().starts_with("app.stats")));
          let reply = if denied {
            json!({"jsonrpc":"2.0","id":request["id"],"error":{"code":-32603,"message":"private server details"}})
          } else {
            json!({"jsonrpc":"2.0","id":request["id"],"result":result})
          };
          if socket.send(Message::Text(reply.to_string().into())).await.is_err() {
            break;
          }
          if method == "core.subscribe" && !denied {
            let collection = request["params"][0].as_str().unwrap();
            assert!(
              collection.ends_with(&format!("{{\"interval\":{interval}}}")),
              "collection must use the current interval with the API minimum"
            );
            let fields = if collection.starts_with("reporting.realtime") {
              json!({
                "cpu":{"cpu":{"usage":25,"temp":50}},
                "memory":{"physical_memory_total":1000,"physical_memory_available":400,"arc_size":200},
                "interfaces":{"eth0":{"link_state":"LINK_STATE_UP","received_bytes_rate":100,"sent_bytes_rate":50}}
              })
            } else {
              json!([{"app_name":"postgres","cpu_usage":2.5,"memory":100,
              "networks":[],"blkio":{"read":10,"write":20}}])
            };
            if socket
              .send(Message::Text(
                json!({"method":"collection_update",
            "params":{"collection":collection,"fields":fields}})
                .to_string()
                .into(),
              ))
              .await
              .is_err()
            {
              break;
            }
          }
        }
      }
    });
    (
      Server::new("test".into(), format!("https://{address}").parse().unwrap())
        .unwrap()
        .with_certificate_sha256(pin),
      task,
    )
  }

  mod client {
    use super::*;

    mod call {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_preserves_events_between_replies_and_redacts_rpc_errors() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let peer = tokio::spawn(async move {
          let (stream, _) = listener.accept().await.unwrap();
          let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
          let request: Value = serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
          assert_eq!(request["method"], "app.query");
          socket.send(Message::Text(json!({"jsonrpc":"2.0","method":"collection_update","params":{"collection":"reporting.realtime","fields":{"cpu":{}}}}).to_string().into())).await.unwrap();
          socket
            .send(Message::Text(
              json!({"jsonrpc":"2.0","id":request["id"],"result":[]})
                .to_string()
                .into(),
            ))
            .await
            .unwrap();
          let request: Value = serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
          socket
            .send(Message::Text(
              json!({"jsonrpc":"2.0","id":request["id"],"error":{"code":-32001,"message":"sensitive-server-detail"}})
                .to_string()
                .into(),
            ))
            .await
            .unwrap();
          socket.close(None).await.unwrap();
        });
        let (socket, _) = tokio_tungstenite::connect_async(format!("ws://{address}"))
          .await
          .unwrap();
        let mut client = Client {
          id: 0,
          pending: VecDeque::new(),
          socket,
        };
        assert_eq!(client.call("app.query", json!([])).await.unwrap(), json!([]));
        assert_eq!(client.next().await.unwrap()["method"], "collection_update");
        let error = client.call("app.query", json!([])).await.unwrap_err();
        assert!(error.to_string().contains("-32001"));
        assert!(!error.to_string().contains("sensitive-server-detail"));
        assert!(matches!(client.next().await, Err(Error::Closed)));
        peer.await.unwrap();
      }
    }
    mod connect {
      use super::*;

      #[tokio::test]
      async fn it_authenticates_over_http_without_reading_certificates() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let peer = tokio::spawn(async move {
          let (stream, _) = listener.accept().await.unwrap();
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
        });
        let server = Server::new("test".into(), format!("http://{address}").parse().unwrap())
          .unwrap()
          .with_ca_cert("/nonexistent/tntop-test-ca.pem".into());
        let client = Client::connect(&server, "test-api-key").await.unwrap();
        assert!(matches!(client.socket.get_ref(), MaybeTlsStream::Plain(_)));
        peer.await.unwrap();
      }

      #[tokio::test]
      async fn it_initializes_tls_and_reports_a_failed_handshake() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let peer = tokio::spawn(async move {
          let (stream, _) = listener.accept().await.unwrap();
          tokio::time::sleep(Duration::from_millis(20)).await;
          drop(stream);
        });
        let server = Server::new("test".into(), format!("https://{address}").parse().unwrap()).unwrap();
        assert!(matches!(
          Client::connect(&server, "not-a-real-key").await,
          Err(Error::WebSocket(_))
        ));
        peer.await.unwrap();
      }
    }
  }

  mod state {
    use super::*;

    mod record_metrics {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_counts_only_observed_time_and_keeps_peaks_across_reconnects() {
        let mut state = State::default();
        let now = Instant::now();
        state.record_metrics(fixtures::metrics(10.0), now, None);
        state.record_metrics(
          fixtures::metrics(20.0),
          now + Duration::from_secs(2),
          Some(Duration::from_secs(2)),
        );
        assert_eq!(state.network_totals()["enp1s0"].downloaded(), 40_000_000.0);
        state.record_metrics(fixtures::metrics(5.0), now + Duration::from_secs(100), None);
        assert_eq!(state.network_totals()["enp1s0"].downloaded(), 40_000_000.0);
        assert_eq!(state.network_totals()["enp1s0"].peak_download(), 20_000_000.0);
      }
    }
  }

  mod version {
    use super::*;

    mod apps {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_does_not_mistake_missing_update_data_for_current() {
        assert!(
          Version::V25_10
            .apps(json!([{"name":"custom","state":"STOPPED","upgrade_available":false}]))
            .is_err()
        );
      }

      #[test]
      fn it_reports_catalog_and_image_updates() {
        for (catalog, image, expected) in [
          (true, false, "available"),
          (false, true, "available"),
          (false, false, "current"),
        ] {
          let apps = Version::V25_10.apps(json!([{"name":"jellyfin","state":"RUNNING","upgrade_available":catalog,"image_updates_available":image}])).unwrap();
          assert_eq!(apps[0].update(), expected);
        }
      }
    }
  }

  mod watch {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_changes_live_intervals_without_losing_history_or_leaving_a_collector() {
      let (server, peer) = server(true, true, vec![2, 4]).await;
      let (mut receiver, interval, task) = watch(server, Zeroizing::new("test-api-key".into()), 100);
      timeout(Duration::from_secs(5), async {
        while receiver.borrow().history().is_empty() {
          receiver.changed().await.unwrap();
        }
        let first = receiver.borrow().history()[0].clone();
        interval.send_replace(3500);
        while receiver.borrow().history().len() < 2 {
          receiver.changed().await.unwrap();
        }
        assert!(Arc::ptr_eq(&first, &receiver.borrow().history()[0]));
      })
      .await
      .unwrap();
      drop(receiver);
      timeout(Duration::from_secs(5), task).await.unwrap().unwrap();
      timeout(Duration::from_secs(5), peer).await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn it_keeps_metrics_when_optional_api_permissions_are_denied() {
      let (server, peer) = server(true, false, vec![2]).await;
      let (mut receiver, _interval, task) = watch(server, Zeroizing::new("test-api-key".into()), 100);
      let state = timeout(Duration::from_secs(5), async {
        loop {
          receiver.changed().await.unwrap();
          let state = receiver.borrow().clone();
          if state.updated_at().is_some()
            && state.apps_error().is_some()
            && state.system_error().is_some()
            && state.app_stats_error().is_some()
          {
            break state;
          }
        }
      })
      .await
      .unwrap();
      assert!(state.error().is_none());
      assert!(state.apps().is_none());
      assert!(state.apps_error().as_ref().unwrap().contains("app.query"));
      assert!(!state.apps_error().as_ref().unwrap().contains("private server details"));
      assert_eq!(state.history().back().unwrap().cpu()["cpu"].usage(), 25.0);
      drop(receiver);
      timeout(Duration::from_secs(5), task).await.unwrap().unwrap();
      timeout(Duration::from_secs(5), peer).await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn it_publishes_live_metrics_apps_and_system_details_and_stops_when_closed() {
      let (server, peer) = server(true, true, vec![2]).await;
      let (mut receiver, _interval, task) = watch(server, Zeroizing::new("test-api-key".into()), 100);
      let state = timeout(Duration::from_secs(5), async {
        loop {
          receiver.changed().await.unwrap();
          let state = receiver.borrow().clone();
          if state.updated_at().is_some()
            && state.apps().is_some()
            && state.system_info().is_some()
            && !state.app_history().is_empty()
          {
            break state;
          }
        }
      })
      .await
      .unwrap();
      assert_eq!(state.version(), "TrueNAS-25.10.8");
      assert_eq!(state.system_info().as_ref().unwrap().model(), "Test CPU");
      assert_eq!(state.apps().as_ref().unwrap()[0].update(), "available");
      assert_eq!(*state.app_history().back().unwrap()["postgres"].cpu_usage(), 2.5);
      assert_eq!(state.network_totals()["eth0"].downloaded(), 0.0);
      assert!(state.error().is_none());
      drop(receiver);
      timeout(Duration::from_secs(5), task).await.unwrap().unwrap();
      timeout(Duration::from_secs(5), peer).await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn it_stops_retrying_a_rejected_api_key() {
      let (server, peer) = server(false, true, vec![2]).await;
      let (mut receiver, _interval, task) = watch(server, Zeroizing::new("test-api-key".into()), 100);
      timeout(Duration::from_secs(5), receiver.changed())
        .await
        .unwrap()
        .unwrap();
      assert!(receiver.borrow().error().as_ref().unwrap().contains("API key rejected"));
      timeout(Duration::from_secs(5), task).await.unwrap().unwrap();
      assert!(receiver.changed().await.is_err());
      timeout(Duration::from_secs(5), peer).await.unwrap().unwrap();
    }
  }
}
