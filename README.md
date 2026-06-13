# Ping Monitor

A network latency monitor for the Linux desktop, written in Rust with
GTK4 / libadwaita. It pings a configurable set of hosts, scores each
one's connection quality, and shows the history as live graphs.

## Features

- **Per-host quality score (0–5)** derived from latency, jitter and packet loss.
- **Status detection** — online / degraded / offline, with configurable thresholds.
- **Three live views** — sortable table, tiles, and a per-host route view.
- **History graph** with selectable hosts, backed by SQLite (30-day retention,
  pruned on startup).
- **Settings** — hosts, ping interval, quality and degraded thresholds, all
  persisted to `config.json`.
- **Multilingual UI** (language switch applies on restart).

## Build & run

Requires GTK4 (≥ 4.14) and libadwaita (≥ 1.5) development packages.

```sh
cargo run --release
```

## License

Copyright (C) 2026 WhiteWolf832.

Released under the **GNU General Public License v3.0 or later** — see
[LICENSE](LICENSE). This program comes with ABSOLUTELY NO WARRANTY.
