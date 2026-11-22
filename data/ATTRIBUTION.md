# Data attribution

`lifetable.bin` is derived from:

> United Nations, Department of Economic and Social Affairs, Population Division (2024).
> *World Population Prospects 2024*, Complete life tables (single ages 0–100+), Medium variant,
> 2024–2100. <https://population.un.org/wpp/>

© United Nations, licensed under [CC BY 3.0 IGO](https://creativecommons.org/licenses/by/3.0/igo/).

Changes made: only the death rates `mx` for the years 2024, 2050 and 2100 are kept for the
237 countries/areas and the World aggregate; values are quantised to 16 bits on a log scale
(relative precision ≈ 0.02 %). Rebuild with `cargo run -p lifetable-builder --release`.

The United Nations does not endorse this application or its estimates.
