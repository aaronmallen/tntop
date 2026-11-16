use std::process::{Command, Output, Stdio};

#[cfg(test)]
mod tests {
  use super::*;

  fn run(directory: &std::path::Path, arguments: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tntop"));
    for (name, _) in std::env::vars_os() {
      if name.to_string_lossy().starts_with("TNTOP_") {
        command.env_remove(name);
      }
    }
    command
      .args(["--config", directory.join("config.toml").to_str().unwrap()])
      .args(arguments)
      .env("NO_COLOR", "1")
      .stdin(Stdio::null())
      .output()
      .unwrap()
  }

  mod startup {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_validates_startup_presets_before_opening_the_terminal() {
      let directory = tempfile::tempdir().unwrap();
      let output = run(directory.path(), &["--preset", "3"]);
      assert_eq!(output.status.code(), Some(2));
      assert!(String::from_utf8_lossy(&output.stderr).contains("requires an interactive terminal"));
      let output = run(directory.path(), &["--preset", "9"]);
      assert_eq!(output.status.code(), Some(78));
      assert!(String::from_utf8_lossy(&output.stderr).contains("preset 9 does not exist"));
      std::fs::write(directory.path().join("config.toml"), "disable_presets = 'custom'\n").unwrap();
      let output = run(directory.path(), &["--preset", "1"]);
      assert_eq!(output.status.code(), Some(78));
      assert!(String::from_utf8_lossy(&output.stderr).contains("preset 1 is disabled"));
    }

    #[test]
    fn it_rejects_invalid_config_before_starting_the_ui() {
      let directory = tempfile::tempdir().unwrap();
      std::fs::write(directory.path().join("config.toml"), "update_ms = 0\n").unwrap();
      let output = run(directory.path(), &[]);
      assert_eq!(output.status.code(), Some(78));
      assert!(String::from_utf8_lossy(&output.stderr).contains("update_ms must be"));
    }

    #[test]
    fn it_rejects_removed_subcommands() {
      let directory = tempfile::tempdir().unwrap();
      let output = run(directory.path(), &["server", "add", "nas", "https://nas.example.com"]);
      assert_eq!(output.status.code(), Some(2));
      assert!(!directory.path().join("config.toml").exists());
    }

    #[test]
    fn it_rejects_unknown_server_names_before_opening_the_terminal() {
      let directory = tempfile::tempdir().unwrap();
      let output = run(directory.path(), &["--server", "missing"]);
      assert_eq!(output.status.code(), Some(78));
      assert!(String::from_utf8_lossy(&output.stderr).contains("no configured server named missing"));
    }

    #[test]
    fn it_requires_a_terminal_even_on_first_run() {
      let directory = tempfile::tempdir().unwrap();
      let output = run(directory.path(), &[]);
      assert_eq!(output.status.code(), Some(2));
      assert!(String::from_utf8_lossy(&output.stderr).contains("requires an interactive terminal"));
      assert!(!directory.path().join("config.toml").exists());
    }

    #[test]
    fn it_selects_a_named_server_before_loading_credentials() {
      let directory = tempfile::tempdir().unwrap();
      std::fs::write(
        directory.path().join("config.toml"),
        "[[servers]]\nname='nas'\nurl='https://nas.example.com'\n",
      )
      .unwrap();
      let output = run(directory.path(), &["--debug", "--server", "nas"]);
      assert_eq!(output.status.code(), Some(2));
      let error = String::from_utf8_lossy(&output.stderr);
      assert!(error.contains("requires an interactive terminal"));
      assert!(error.contains("Loading configuration"));
      assert!(!error.contains("API key"));
    }

    #[test]
    fn it_shows_flags_and_version_without_loading_invalid_config() {
      let directory = tempfile::tempdir().unwrap();
      std::fs::write(directory.path().join("config.toml"), "invalid toml [").unwrap();
      for (arguments, expected) in [
        (vec!["--help"], "--server"),
        (vec!["--version"], env!("CARGO_PKG_VERSION")),
      ] {
        let output = run(directory.path(), &arguments);
        assert!(output.status.success(), "{output:?}");
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(text.contains(expected));
        assert!(!text.contains("Commands:"));
        assert!(output.stderr.is_empty());
      }
    }
  }
}
