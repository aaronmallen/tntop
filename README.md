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
and `TTY` always mean the built-in palettes. Invalid user files report an error rather than
silently loading a different theme. Names are case-sensitive.

Explicit paths still work: `theme = "themes/personal.theme"` resolves relative to
the selected config file, and absolute paths remain absolute.

## Graphs

**Options → General → Graph symbol** cycles `braille`, `block` and `tty`.
Each panel category has an override; `default` inherits the General choice.
Braille uses four vertical dots and block uses two half-cell rows, both with
two samples per column. TTY uses one ASCII cell per sample. Switching symbols
changes how much history fits, without discarding the collected history.

Turn off **Truecolor** in General for 256-color output. **Force TTY** uses the
built-in 16-color TTY palette, ASCII text and TTY graphs across the whole UI.
Turning it off restores your selected theme and graph choices. Selecting the
`TTY` theme also enables TTY output.

## Panels

Press **1–4** to toggle CPU, Memory/ZFS, Network and Apps. Each panel's Options
category also has a **Show panel** setting. CPU can sit above or below the other
panels, Apps can sit on either side, and Memory/ZFS can sit above or below Network.
Storage stays inside Memory/ZFS. Hidden panels keep their sessions and history;
the menu remains available when every panel is hidden.

## Preset definitions

The `presets` setting holds up to nine space-separated layouts. Each layout
contains comma-separated `panel:position:graph` entries. Panels are `cpu`, `mem`,
`net` and `apps`; `proc` aliases `apps`. Positions are `0` or `1`; `1` puts CPU
at the bottom, Memory below Network, or Apps on the left. Network follows Memory's
position, as in btop. Graphs accept `default`, `braille`, `block` or `tty`.

Preset 0 always shows all four panels in their default positions. The three
configured defaults are:

```toml
presets = "cpu:1:default,proc:0:default cpu:0:default,mem:0:default,net:0:default cpu:0:block,net:0:tty"
```

Edit definitions in General Options or the config file. Invalid definitions
leave the live settings intact. An empty string leaves only preset 0.

Press **p/P** or click the header arrows to cycle forward/back. The header shows
the active preset, or **manual** after a layout change. `tntop --preset 2` selects
a preset at startup. **Disable presets** in General accepts `off`, `default`
(preset 0), `custom` (presets 1–9) or `all`. Cycling skips disabled choices;
startup selection reports an error for a disabled or missing preset.

## Appearance and input

General Options controls rounded corners, synchronized terminal output,
Vim keys, byte units and the clock. TTY mode uses ASCII borders and disables
synchronized output. Turn synchronization off if your terminal does not
support it.

With Vim keys enabled, **h/j/k/l** act as Left/Down/Up/Right in lists and
menus. **H** still opens Help. Text fields and the app filter keep literal
letters.

**Base-10 sizes** uses kB/MB/GB (1000 bytes per step); otherwise displays use
KiB/MiB/GiB (1024). This applies to memory, storage, network and apps.

**Clock format** accepts UTC strftime formats such as `%H:%M:%S UTC`.
An empty format hides the clock. `/host` uses the configured server URL
host, `/user` shows `N/A` because API identity is unavailable, and `/uptime`
uses the existing TrueNAS uptime reading.

## Refresh and logging

Set **Redraw interval (ms)** in Options or press **+/-** on the dashboard to
change redraw timing by 100 ms, within 100..86400000 ms. Collection has a
2000 ms floor; TrueNAS stream intervals round up to whole seconds. Changes
above that floor reconnect the existing collector with its history intact.
Faster painting does not produce more server data.

Turn **Background update** off to freeze the dashboard behind menus.
Collection continues, and resizing redraws the background once.

**Log level** accepts OFF, ERROR, WARN, INFO, DEBUG and TRACE. During the
TUI, logs append to `tntop.log` beside the config file. They do not print
over the display. Exit and error paths restore the prior logging state.

## CPU display

CPU Options selects each graph from total usage, the busiest core, or a
core available on the current server. **Auto** means total for the upper
graph and busiest core for the lower. A saved core missing on another
server shows **N/A**. Single-graph mode shows only the upper source.
The lower graph can point up or down.

Show or hide CPU/core temperatures and uptime, choose Celsius, Fahrenheit,
Kelvin or Rankine, and set a custom CPU name. An empty name uses the model
from TrueNAS. Missing temperatures stay **N/A**. These settings use the
readings already collected.

## Memory and ZFS

**Memory graphs** switches between history graphs and current-value meters.
Both use total RAM as their scale. **Show storage** hides or shows pools and
disk I/O; memory uses the whole panel when storage is hidden.

**Show ARC/cache** adds separate ARC-size and reported ARC-free rows.
Used RAM remains total minus available. ARC overlaps RAM, so tntop never
adds it to Used or Available or guesses reclaimable RAM. Turn this option
off for RAM-only display. Missing ARC-free readings show **N/A**.

## Storage

**Pool filter** accepts space-separated name fragments. Prefix `exclude=`
to hide matches; empty shows all pools. **[ / ]** cycles matching pools,
keeping selection by name as pools change.

**Show disk I/O** toggles aggregate statistics. **I/O view** replaces pool
capacity with read/write graphs. Combine read and write or show read above
write on a shared scale. **I/O scale (bytes/s)** sets a fixed maximum;
zero uses retained history. Capacity belongs to each pool, but disk busy,
read/write rates and IOPS always describe **all disks**.

## Network

Set a **Preferred interface** or use **b/n** to cycle and save one.
If it is absent on another server, tntop uses an up interface, then the
first available, while keeping your preference.

**a** toggles automatic/fixed scaling; **y** gives both directions the
larger scale. Set fixed maxima in bytes/s in Network Options. Swap graph
positions or enable bitrate units there. Bitrates use decimal or binary
units from General Options; totals stay in bytes.

**z** sets a local baseline for the current server/interface. Press it
again to show the full session estimate. Labels keep `~` because these
totals come from sampled rates. This never changes server counters.

## LICENSE

tntop is licensed under the [AGPLv3 License]. Copyright © 2026 [Aaron Allen].

[Aaron Allen]: https://aaronmallen.me
[AGPLv3 License]: LICENSE
[TrueNAS]: https://www.truenas.com/
[TUI]: https://en.wikipedia.org/wiki/Text-based_user_interface
