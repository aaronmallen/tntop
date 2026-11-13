use std::collections::BTreeMap;

use serde_json::Value;

use super::{AppStats, Error, Metrics};

pub(super) fn app_stats(value: Value) -> Result<BTreeMap<String, AppStats>, Error> {
  let stats: Vec<AppStats> = serde_json::from_value(value)?;
  if stats
    .iter()
    .any(|stats| !stats.cpu_usage().is_finite() || *stats.cpu_usage() < 0.0)
  {
    return Err(Error::Protocol("app stats contain invalid CPU usage".into()));
  }
  Ok(
    stats
      .into_iter()
      .map(|stats| (stats.app_name().clone(), stats))
      .collect(),
  )
}

pub(super) fn metrics(value: Value) -> Result<Metrics, Error> {
  let metrics: Metrics = serde_json::from_value(value)?;
  if !metrics.cpu().contains_key("cpu") || metrics.memory().physical_memory_total() == 0 {
    return Err(Error::Protocol("metrics omit aggregate CPU or total memory".into()));
  }
  if metrics
    .cpu()
    .values()
    .any(|cpu| !cpu.usage().is_finite() || !(0.0..=100.0).contains(&cpu.usage()))
    || metrics.interfaces().values().any(|net| {
      !net.received_bytes_rate().is_finite()
        || net.received_bytes_rate() < 0.0
        || !net.sent_bytes_rate().is_finite()
        || net.sent_bytes_rate() < 0.0
    })
    || metrics.memory().physical_memory_available() > metrics.memory().physical_memory_total()
    || metrics.memory().arc_size() > metrics.memory().physical_memory_total()
    || metrics
      .memory()
      .arc_free_memory()
      .is_some_and(|free| free > metrics.memory().physical_memory_total())
    || metrics.pools().values().any(|pool| {
      !pool.total().is_finite()
        || pool.total() <= 0.0
        || !pool.used().is_finite()
        || pool.used() < 0.0
        || !pool.available().is_finite()
        || pool.available() < 0.0
    })
  {
    return Err(Error::Protocol("metrics contain out-of-range values".into()));
  }
  Ok(metrics)
}

#[cfg(test)]
mod tests {
  use serde_json::json;

  use super::*;

  pub(crate) fn sample() -> Value {
    json!({
      "cpu": {"cpu": {"usage": 32.5}, "cpu0": {"usage": 65.0, "temp": 51.0}, "cpu1": {"usage": 0.0, "temp": null}},
      "interfaces": {"en0": {"link_state": "LINK_STATE_UP", "received_bytes_rate": 2048.0, "sent_bytes_rate": 1024.0},
        "en1": {"link_state": "LINK_STATE_DOWN", "received_bytes": 0, "sent_bytes": 0}},
      "memory": {"arc_size": 8192, "physical_memory_available": 16384, "physical_memory_total": 32768},
      "zfs": {"demand_accesses_per_second":1000, "demand_data_hits_per_second":600, "demand_metadata_hits_per_second":390},
      "disks": {"busy":25,"read_bytes":4096,"write_bytes":2048,"read_ops":4,"write_ops":2},
      "pools": {"tank":{"total":100000,"used":40000,"available":60000}}
    })
  }

  mod app_stats {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_accepts_fractional_cpu_and_aggregates_app_networks() {
      let stats = super::super::app_stats(json!([{
        "app_name":"postgres","cpu_usage":0.0096375,"memory":224006144,
        "networks":[{"rx_bytes":100,"tx_bytes":20},{"rx_bytes":50,"tx_bytes":30}],
        "blkio":{"read":255242240,"write":0}
      }]))
      .unwrap();
      assert_eq!(*stats["postgres"].cpu_usage(), 0.0096375);
      assert_eq!(stats["postgres"].download(), 150.0);
      assert_eq!(stats["postgres"].upload(), 50.0);
      assert_eq!(stats["postgres"].blkio().read(), 255242240);
    }
  }

  mod metrics {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_decodes_cpu_arc_and_interface_rates() {
      let metrics = super::super::metrics(sample()).unwrap();
      assert_eq!(metrics.cpu()["cpu"].usage(), 32.5);
      assert_eq!(metrics.cpu()["cpu1"].temp(), None);
      assert_eq!(metrics.memory().arc_size(), 8192);
      assert_eq!(metrics.interfaces()["en0"].received_bytes_rate(), 2048.0);
      assert_eq!(metrics.interfaces()["en1"].received_bytes_rate(), 0.0);
      assert_eq!(metrics.disks().as_ref().unwrap().read_bytes(), 4096.0);
      assert_eq!(metrics.pools()["tank"].available(), 60000.0);
      assert_eq!(metrics.zfs().as_ref().unwrap().hit_ratio(), Some(0.99));
    }

    #[test]
    fn it_rejects_missing_or_invalid_measurements() {
      for pointer in ["/cpu/cpu/usage", "/memory/arc_size", "/interfaces/en0/sent_bytes_rate"] {
        let mut value = sample();
        *value.pointer_mut(pointer).unwrap() = json!(-1);
        assert!(super::super::metrics(value).is_err());
      }
      let mut value = sample();
      value["cpu"].as_object_mut().unwrap().remove("cpu");
      assert!(super::super::metrics(value).is_err());
    }
  }
}
