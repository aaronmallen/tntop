use super::*;

pub(crate) fn metrics(usage: f64) -> Metrics {
  Version::V25_10.metrics(json!({
    "cpu": {"cpu":{"usage":usage,"temp":62}, "cpu0":{"usage":usage,"temp":61},
      "cpu1":{"usage":usage*0.8,"temp":58}, "cpu2":{"usage":usage*0.9,"temp":59}, "cpu3":{"usage":usage*0.7,"temp":60}},
    "memory": {"physical_memory_total":17179869184u64,"physical_memory_available":4294967296u64,
      "arc_size":8589934592u64,"arc_free_memory":1073741824u64},
    "interfaces": {"enp1s0":{"link_state":"LINK_STATE_UP","speed":2500,
      "received_bytes_rate":usage*1000000.0,"sent_bytes_rate":usage*20000.0}},
    "disks": {"busy":usage,"read_bytes":usage*500000.0,"write_bytes":usage*100000.0,"read_ops":100,"write_ops":250},
    "pools": {"boot-pool":{"total":256000000000u64,"used":24000000000u64,"available":232000000000u64},
      "storage_share":{"total":8000000000000u64,"used":550000000000u64,"available":7450000000000u64}},
    "zfs":{"demand_accesses_per_second":1000,"demand_data_hits_per_second":600,"demand_metadata_hits_per_second":390}
  })).unwrap()
}

pub(crate) fn state() -> State {
  let now = Instant::now();
  let mut state = State {
    apps: Some(Version::V25_10.apps(json!([
      {"name":"forgejo","state":"RUNNING","image_updates_available":false,"upgrade_available":true,"human_version":"17.0.1"},
      {"name":"postgres","state":"RUNNING","image_updates_available":false,"upgrade_available":false,"human_version":"18.0"},
      {"name":"jellyfin","state":"STOPPED","image_updates_available":false,"upgrade_available":false,"human_version":"10.11"}
    ])).unwrap()),
    apps_updated_at: Some(now),
    app_stats_updated_at: Some(now),
    system_info: Some(Version::V25_10.system_info(json!({"model":"Intel(R) N150","loadavg":[4.45,3.94,3.41],"uptime_seconds":10162.0})).unwrap()),
    system_updated_at: Some(now),
    version: "TrueNAS-25.10.8".into(),
    ..State::default()
  };
  for index in 0..160 {
    let usage = 25.0 + (index as f64 / 10.0).sin() * 20.0;
    state.record_metrics(
      metrics(usage),
      now,
      if index == 0 { None } else { Some(Duration::from_secs(2)) },
    );
    let stats = Version::V25_10.app_stats(json!([
      {"app_name":"forgejo","cpu_usage":usage/2.0,"memory":400000000,"networks":[{"interface_name":"eth0","rx_bytes":536642,"tx_bytes":521223}],"blkio":{"read":402984960,"write":10000000}},
      {"app_name":"postgres","cpu_usage":0.0096375,"memory":224006144,"networks":[],"blkio":{"read":255242240,"write":0}},
      {"app_name":"jellyfin","cpu_usage":0,"memory":0,"networks":[],"blkio":{"read":0,"write":0}}
    ])).unwrap();
    state.app_history.push_back(Arc::new(stats));
  }
  state
}

pub(crate) fn without_app_stats() -> State {
  let mut state = state();
  state.app_history.clear();
  state.app_stats_updated_at = None;
  state.app_stats_error = Some("app.stats requires APPS_READ".into());
  state
}
