use super::{Error, GraphSymbol, Settings};

impl Settings {
  pub fn presets(&self) -> Result<Vec<Self>, Error> {
    let definitions: Vec<_> = self.presets.split_whitespace().collect();
    if definitions.len() > 9 {
      return Err(Error::Invalid("presets supports at most nine custom layouts".into()));
    }
    std::iter::once("cpu:0:default,mem:0:default,net:0:default,apps:0:default")
      .chain(definitions)
      .enumerate()
      .map(|(index, definition)| {
        let mut candidate = self.clone();
        candidate.show_cpu = false;
        candidate.show_mem = false;
        candidate.show_net = false;
        candidate.show_apps = false;
        candidate.cpu_bottom = false;
        candidate.mem_below_net = false;
        candidate.apps_left = false;
        candidate.graph_symbol_cpu = GraphSymbol::Default;
        candidate.graph_symbol_mem = GraphSymbol::Default;
        candidate.graph_symbol_net = GraphSymbol::Default;
        candidate.graph_symbol_apps = GraphSymbol::Default;
        for entry in definition.split(',') {
          let parts: Vec<_> = entry.split(':').collect();
          let [panel, position, symbol] = parts.as_slice() else {
            return Err(Error::Invalid(format!(
              "preset {index}: expected panel:position:graph, got {entry:?}"
            )));
          };
          let position = match *position {
            "0" => false,
            "1" => true,
            _ => return Err(Error::Invalid(format!("preset {index}: position must be 0 or 1"))),
          };
          let symbol = match *symbol {
            "default" => GraphSymbol::Default,
            "braille" => GraphSymbol::Braille,
            "block" => GraphSymbol::Block,
            "tty" => GraphSymbol::Tty,
            _ => {
              return Err(Error::Invalid(format!(
                "preset {index}: unknown graph symbol {symbol:?}"
              )));
            }
          };
          let (visible, graph, placement) = match *panel {
            "cpu" => (
              &mut candidate.show_cpu,
              &mut candidate.graph_symbol_cpu,
              Some(&mut candidate.cpu_bottom),
            ),
            "mem" => (
              &mut candidate.show_mem,
              &mut candidate.graph_symbol_mem,
              Some(&mut candidate.mem_below_net),
            ),
            "net" => (&mut candidate.show_net, &mut candidate.graph_symbol_net, None),
            "apps" | "proc" => (
              &mut candidate.show_apps,
              &mut candidate.graph_symbol_apps,
              Some(&mut candidate.apps_left),
            ),
            _ => {
              return Err(Error::Invalid(format!(
                "preset {index}: unsupported panel {panel:?}; use cpu, mem, net or apps"
              )));
            }
          };
          if *visible {
            return Err(Error::Invalid(format!("preset {index}: repeated panel {panel:?}")));
          }
          *visible = true;
          *graph = symbol;
          if let Some(placement) = placement {
            *placement = position;
          }
        }
        Ok(candidate)
      })
      .collect()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn it_parses_defaults_aliases_and_custom_layouts_and_saves_only_valid_definitions() {
    let settings = Settings::default();
    let presets = settings.presets().unwrap();
    assert_eq!(presets.len(), 4);
    assert!(presets[0].show_cpu && presets[0].show_mem && presets[0].show_net && presets[0].show_apps);
    assert!(presets[1].show_cpu && presets[1].show_apps && presets[1].cpu_bottom && !presets[1].show_mem);
    assert!(presets[2].show_mem && presets[2].show_net && !presets[2].show_apps);
    assert_eq!(presets[3].graph_symbol_cpu, GraphSymbol::Block);
    assert_eq!(presets[3].graph_symbol_net, GraphSymbol::Tty);
    let custom = "proc:1:tty,mem:1:block";
    let changed = settings.with_preference("presets", serde_json::json!(custom)).unwrap();
    let presets = changed.presets().unwrap();
    assert!(presets[1].apps_left && presets[1].mem_below_net);
    assert_eq!(presets[1].graph_symbol_apps, GraphSymbol::Tty);
    assert!(!presets[1].show_cpu);
    for invalid in [
      "gpu:0:default",
      "cpu:2:braille",
      "cpu:0:nope",
      "cpu",
      "cpu:0:tty:extra",
      "cpu:0:tty,",
      "apps:0:tty,proc:1:block",
    ] {
      assert!(
        changed.with_preference("presets", serde_json::json!(invalid)).is_err(),
        "{invalid}"
      );
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    super::super::save_preferences(&path, &settings, &changed).unwrap();
    assert_eq!(super::super::load(&path).unwrap().presets, custom);
    for count in [0, 9, 10] {
      let source = vec!["cpu:0:default"; count].join(" ");
      assert_eq!(
        settings.with_preference("presets", serde_json::json!(source)).is_ok(),
        count <= 9
      );
    }
  }
}
