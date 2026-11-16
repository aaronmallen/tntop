# tntop

A [TUI] to monitor resources and apps on your [TrueNAS] server from a terminal.

> [!WARNING]
> tntop is in active development. Features and configuration may change without notice, and you may encounter bugs.

## Install

Bash or Zsh:

```sh
curl -fsSL https://aaronmallen.dev/aaron/tntop/raw/branch/main/assets/install.sh -o /tmp/tntop-install.sh
sh /tmp/tntop-install.sh
```

Fish:

```fish
curl -fsSL https://aaronmallen.dev/aaron/tntop/raw/branch/main/assets/install.sh -o /tmp/tntop-install.sh
sh /tmp/tntop-install.sh
```

PowerShell 7 on Windows:

```powershell
Invoke-WebRequest https://aaronmallen.dev/aaron/tntop/raw/branch/main/assets/install.ps1 -OutFile "$env:TEMP\tntop-install.ps1"
& "$env:TEMP\tntop-install.ps1"
```

The installers select the latest stable release and verify its SHA-256 checksum.
Published targets are macOS ARM64, Linux x86-64 and Windows x86-64. Other platforms
must build from source. Run the installer again to update.

To choose a version or directory, use `sh /tmp/tntop-install.sh 0.1.0 "$HOME/.local/bin"`,
or `& "$env:TEMP\tntop-install.ps1" -Version 0.1.0 -Directory "$env:LOCALAPPDATA\tntop\bin"`.
Defaults are `~/.local/bin` on Unix and `%LOCALAPPDATA%\tntop\bin` on Windows.
The installer prints PATH setup guidance when needed.

Linux needs a running, unlocked Secret Service keyring to store API keys.
See the [ArchWiki keyring setup guide](https://wiki.archlinux.org/title/GNOME/Keyring)
for daemon and session setup, including systems without a desktop.

## Update

Choose **Update** in the main menu to install the latest stable release. It reports
both versions, skips older releases, and checks the archive's SHA-256 checksum
before replacing the executable. It leaves your settings and saved API keys alone.

Self-update needs `curl` and `tar` on PATH, including Windows' built-in versions.
You must have write access to the executable's directory. The updater never asks
for elevated access. On Windows, it arranges cleanup of the running executable
after exit.

## Connections

Add a server using its HTTP or HTTPS origin, such as `https://nas.local` or
`http://nas.local:8080`. Do not include a path, credentials or query string.

HTTPS uses encrypted WebSockets and the configured CA certificate or trusted
certificate fingerprint. HTTP uses plain WebSockets and skips certificate checks
and trust prompts. HTTP sends your API key and server data without encryption.

## Themes

All 49 bundled btop themes work offline, with no theme files to install.
Open **Options → General → Color theme** and press Enter to browse them.
Arrows preview a theme, Enter keeps it, and Escape restores the prior theme.
Accepted choices follow **Save on exit**. **Theme background** switches between
the theme's background and your terminal's background.

Set `theme = "dracula"` in your config to use a bundled name, or use `"Default"`
for tntop's black background. The bundled notices and source are in [themes](themes/README.md).

Put custom `.theme` files in a `themes/` folder beside your selected config file.
A user file takes precedence over a bundled theme with the same name. `Default`
always means the built-in palette. Invalid user files report an error rather than
silently loading a different theme. Names are case-sensitive.

Explicit paths still work: `theme = "themes/personal.theme"` resolves relative to
the selected config file, and absolute paths remain absolute.

## Graphs

**Options → General → Graph symbol** cycles `braille`, `block` and `tty`.
Each panel category has an override; `default` inherits the General choice.
Braille uses four vertical dots and block uses two half-cell rows, both with
two samples per column. TTY uses one ASCII cell per sample. Switching symbols
changes how much history fits, without discarding the collected history.

## LICENSE

tntop is licensed under the [AGPLv3 License]. Copyright © 2026 [Aaron Allen].

[Aaron Allen]: https://aaronmallen.me
[AGPLv3 License]: LICENSE
[TrueNAS]: https://www.truenas.com/
[TUI]: https://en.wikipedia.org/wiki/Text-based_user_interface
