use std::time::SystemTime;

use crate::{client::State, config::Server};

pub(super) fn format(pattern: &str, server: &Server, state: &State, now: SystemTime) -> String {
  let time = chrono::DateTime::<chrono::Utc>::from(now);
  let uptime = state
    .system_info()
    .as_ref()
    .map(|info| {
      let seconds = (info.uptime_seconds()
        + state
          .system_updated_at()
          .map(|time| time.elapsed().as_secs_f64())
          .unwrap_or(0.0))
      .max(0.0) as u64;
      format!(
        "{}d {:02}:{:02}:{:02}",
        seconds / 86400,
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60
      )
    })
    .unwrap_or_else(|| "N/A".into());
  time
    .format(pattern)
    .to_string()
    .replace("/host", server.url().host_str().unwrap_or("N/A"))
    .replace("/user", "N/A")
    .replace("/uptime", &uptime)
}

#[cfg(test)]
mod tests {
  use super::*;

  mod format {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_formats_utc_and_available_server_values_and_hides_an_empty_clock() {
      let server = Server::new("Storage".into(), "https://nas.example.com".parse().unwrap()).unwrap();
      let now = std::time::UNIX_EPOCH + std::time::Duration::from_secs(3661);
      let state = State::default();
      assert_eq!(
        format("%F %T /host /user /uptime", &server, &state, now),
        "1970-01-01 01:01:01 nas.example.com N/A N/A"
      );
      assert_eq!(format("", &server, &state, now), "");
      assert!(format("/uptime", &server, &crate::client::fixtures::state(), now).starts_with("0d 02:49:"));
      assert!(
        crate::config::Settings::default()
          .with_preference("clock_format", serde_json::json!("%Q"))
          .is_err()
      );
    }
  }
}
