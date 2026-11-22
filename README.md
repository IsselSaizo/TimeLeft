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
| Core (life tables, model, ledger) | Rust | planned |
| Android phone | Kotlin + Jetpack Compose (via UniFFI) | planned |
| Wear OS | Kotlin, Compose for Wear OS, complications, tiles | planned |
| iOS / watchOS | SwiftUI (via UniFFI) | groundwork only |

All data stays on the device. No server.

## Legacy version

The original Unity prototype (*DeathClock / LTimeClock*, 2023) is preserved in git history
under the tag [`v1.0-unity`](../../tree/v1.0-unity).

## License

Code: MIT, see [LICENSE](LICENSE).
Bundled life-table data: © United Nations, World Population Prospects 2024, CC BY 3.0 IGO.
