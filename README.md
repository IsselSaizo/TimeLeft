# Time Left

A lifetime countdown inspired by the movie *In Time*: years and days left as digits,
hours / minutes / seconds on an analog dial. Daily habits move the countdown up or down.

> **Not a medical device.** Estimates are based on population statistics
> (UN World Population Prospects life tables + published risk-factor studies)
> and are meant for motivation and fun, not diagnosis or prediction.

## Status

Rewrite in progress. See [PLAN.md](PLAN.md).

| Part | Tech | Status |
|---|---|---|
| Core (life tables, model, ledger) | Rust | ✅ done, with CLI |
| Android phone | Kotlin + Jetpack Compose (via UniFFI) | planned |
| Wear OS | Kotlin, Compose for Wear OS, complications, tiles | planned |
| iOS / watchOS | SwiftUI (via UniFFI) | groundwork only |

All data stays on the device. No server.

How the numbers are computed: [docs/methodology.md](docs/methodology.md).

## Try the core from a terminal

```sh
cargo run -p timeleft-cli -- init --country RUS --sex male --birth 1990-06-15 \
    --smoking current:10 --steps 5000 --height 180 --weight 85 --track cigarettes,steps
cargo run -p timeleft-cli -- status --live
cargo run -p timeleft-cli -- checkin --cigarettes 2 --steps 11000
cargo run -p timeleft-cli -- explain
```

`cargo run -p timeleft-cli -- countries russia` lists country codes. The state is stored in
`%APPDATA%\TimeLeft\state.json` (or `~/.local/share/TimeLeft/`); override with `--state` or
`TIMELEFT_STATE`.

## Repository layout

| Path | What |
|---|---|
| `core/` | `timeleft-core`: life tables, survival model, risk factors, habits, ledger, countdown. No IO. |
| `cli/` | `timeleft` command-line front end |
| `tools/lifetable-builder/` | UN WPP CSV → `data/lifetable.bin` (`cargo run -p lifetable-builder --release`) |
| `data/` | `lifetable.bin` + attribution |
| `testdata/` | WPP reference values and cross-platform test vectors |
| `docs/` | methodology |

`cargo test` runs everything; `TIMELEFT_UPDATE_VECTORS=1 cargo test -p timeleft-core --test vectors`
regenerates the vectors after an intended change.

## Legacy version

The original Unity prototype (*DeathClock / LTimeClock*, 2023) is preserved in git history
under the tag [`v1.0-unity`](../../tree/v1.0-unity).

## License

Code: MIT, see [LICENSE](LICENSE).
Bundled life-table data: © United Nations, World Population Prospects 2024, CC BY 3.0 IGO.
