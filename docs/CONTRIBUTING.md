# Contributing to tntop

Thank you for helping with tntop. Bug reports, fixes, features, docs and questions are all welcome.

## Before You Start

Read the [Code of Conduct] before you take part. By sending a contribution you agree to it, and you agree to
license your work under the [AGPLv3 License][license].

## Development Quickstart

You need [mise]. It installs Rust and every other tool the project uses.

Clone the repo, install its tools and check that everything passes:

```sh
git clone https://aaronmallen.dev/aaron/tntop.git
cd tntop
mise trust
mise install
mise run check
mise run test
mise run lint
```

Run tntop against a scratch config so your own stays untouched:

```sh
mbx run -- --config target/dev.toml --debug
```

## Tasks

Run tools through mise tasks, not bare `cargo` commands. The tasks carry the flags and config paths CI uses.

| Task                     | What it does                                    |
|--------------------------|-------------------------------------------------|
| `mise run build`         | Build the project                               |
| `mise run check`         | Check the code for compile errors               |
| `mise run test`          | Run the tests; add `--filter <name>` to narrow  |
| `mise run test:coverage` | Run the tests and write a report to `coverage/` |
| `mise run format`        | Format Rust, Markdown, TOML, YAML and shell     |
| `mise run lint`          | Lint every file                                 |

Run `mise tasks` to list every task and its aliases.

## Sending a Change

1. Open an issue first for anything larger than a small fix, so we can agree on the approach.
2. Keep each change to one purpose.
3. Add or update tests for the behavior you change.
4. Run `mise run format` and `mise run lint`, then `mise run test`, before you push.
5. Add a line under `Unreleased` in the [changelog][changelog] when users will notice the change.

## Reporting Bugs

[Open an issue][new-issue] with:

- your tntop version (`tntop --version`)
- your TrueNAS version
- your OS and terminal
- the steps that cause the bug, what you expected and what happened

Run tntop with `--debug` for more log output. Remove API keys and hostnames before you paste logs.

## Questions

If something is unclear, [open an issue][new-issue] and ask.

[changelog]: ../CHANGELOG.md
[Code of Conduct]: CODE_OF_CONDUCT.md
[license]: ../LICENSE
[mise]: https://mise.jdx.dev/
[new-issue]: https://aaronmallen.dev/aaron/tntop/issues/new
