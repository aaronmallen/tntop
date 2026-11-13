use std::collections::BTreeMap;

use getset::{CopyGetters, Getters};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
struct AppNetwork {
  rx_bytes: f64,
  tx_bytes: f64,
}

#[derive(Clone, Debug, Deserialize, Getters)]
#[get = "pub"]
pub struct AppStats {
  app_name: String,
  blkio: BlockIo,
  cpu_usage: f64,
  memory: u64,
  networks: Vec<AppNetwork>,
}

impl AppStats {
  pub fn download(&self) -> f64 {
    self
      .networks
      .iter()
      .fold(0.0, |total, net| total + net.rx_bytes.max(0.0))
  }

  pub fn upload(&self) -> f64 {
    self
      .networks
      .iter()
      .fold(0.0, |total, net| total + net.tx_bytes.max(0.0))
  }
}

#[derive(Clone, Debug, Deserialize, Getters)]
#[get = "pub"]
pub struct Application {
  #[serde(default)]
  human_version: Option<String>,
  image_updates_available: bool,
  name: String,
  state: String,
  upgrade_available: bool,
}

impl Application {
  pub fn update(&self) -> &'static str {
    if self.upgrade_available || self.image_updates_available {
      "available"
    } else {
      "current"
    }
  }
}

#[derive(Clone, CopyGetters, Debug, Deserialize)]
#[get_copy = "pub"]
pub struct BlockIo {
  read: u64,
  write: u64,
}

#[derive(Clone, CopyGetters, Debug, Deserialize)]
#[get_copy = "pub"]
pub struct Cpu {
  temp: Option<f64>,
  usage: f64,
}

#[derive(Clone, CopyGetters, Debug, Deserialize)]
#[get_copy = "pub"]
pub struct Disks {
  busy: f64,
  read_bytes: f64,
  read_ops: f64,
  write_bytes: f64,
  write_ops: f64,
}

#[derive(Clone, CopyGetters, Debug, Deserialize, Getters)]
pub struct Interface {
  #[get = "pub"]
  link_state: String,
  #[get_copy = "pub"]
  #[serde(default)]
  received_bytes_rate: f64,
  #[get_copy = "pub"]
  #[serde(default)]
  sent_bytes_rate: f64,
  #[get_copy = "pub"]
  #[serde(default)]
  speed: Option<f64>,
}

#[derive(Clone, CopyGetters, Debug, Deserialize)]
#[get_copy = "pub"]
pub struct Memory {
  #[serde(default)]
  arc_free_memory: Option<u64>,
  arc_size: u64,
  physical_memory_available: u64,
  physical_memory_total: u64,
}

#[derive(Clone, Debug, Deserialize, Getters)]
#[get = "pub"]
pub struct Metrics {
  cpu: BTreeMap<String, Cpu>,
  #[serde(default)]
  disks: Option<Disks>,
  interfaces: BTreeMap<String, Interface>,
  memory: Memory,
  #[serde(default)]
  pools: BTreeMap<String, Pool>,
  #[serde(default)]
  zfs: Option<Zfs>,
}

#[derive(Clone, CopyGetters, Debug, Default)]
#[get_copy = "pub"]
pub struct NetworkTotals {
  downloaded: f64,
  peak_download: f64,
  peak_upload: f64,
  uploaded: f64,
}

impl NetworkTotals {
  pub(super) fn record(&mut self, interface: &Interface, elapsed: Option<std::time::Duration>) {
    self.peak_download = self.peak_download.max(interface.received_bytes_rate);
    self.peak_upload = self.peak_upload.max(interface.sent_bytes_rate);
    if let Some(elapsed) = elapsed {
      self.downloaded += interface.received_bytes_rate * elapsed.as_secs_f64();
      self.uploaded += interface.sent_bytes_rate * elapsed.as_secs_f64();
    }
  }
}

#[derive(Clone, CopyGetters, Debug, Deserialize)]
#[get_copy = "pub"]
pub struct Pool {
  available: f64,
  total: f64,
  used: f64,
}

#[derive(Clone, Debug, Deserialize, Getters)]
#[get = "pub"]
pub struct SystemInfo {
  loadavg: [f64; 3],
  model: String,
  uptime_seconds: f64,
}

#[derive(Clone, CopyGetters, Debug, Deserialize)]
#[get_copy = "pub"]
pub struct Zfs {
  demand_accesses_per_second: f64,
  demand_data_hits_per_second: f64,
  demand_metadata_hits_per_second: f64,
}

impl Zfs {
  pub fn hit_ratio(&self) -> Option<f64> {
    (self.demand_accesses_per_second > 0.0).then(|| {
      ((self.demand_data_hits_per_second + self.demand_metadata_hits_per_second) / self.demand_accesses_per_second)
        .clamp(0.0, 1.0)
    })
  }
}
