# tntop

A [TUI] to monitor resources and apps on your [TrueNAS] server from a terminal.

> [!WARNING]
> tntop is in active development. Features and configuration may change without notice, and you may encounter bugs.

## Connections

Add a server using its HTTP or HTTPS origin, such as `https://nas.local` or
`http://nas.local:8080`. Do not include a path, credentials or query string.

HTTPS uses encrypted WebSockets and the configured CA certificate or trusted
certificate fingerprint. HTTP uses plain WebSockets and skips certificate checks
and trust prompts. HTTP sends your API key and server data without encryption.

## LICENSE

tntop is licensed under the [AGPLv3 License]. Copyright © 2026 [Aaron Allen].

[Aaron Allen]: https://aaronmallen.me
[AGPLv3 License]: LICENSE
[TrueNAS]: https://www.truenas.com/
[TUI]: https://en.wikipedia.org/wiki/Text-based_user_interface
