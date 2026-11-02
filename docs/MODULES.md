# solana-agent capability modules (phase 4, SIMULATED ONLY)

`src/modules/` defines a `Module` trait with `describe`, `simulate` and `execute`. `execute` always refuses. Nothing here signs, sends or trades, and nothing touches mainnet.

| Module | Simulation | Notes |
|---|---|---|
| `pump-fun` | bonding-curve buy quote: fee first, then x·y=k over virtual reserves | Program ids `6EF8rrec…`, `pAMMBay6…` and `pfeeUxB6…` and the curve layout come from pump-fun/pump-public-docs. Fee bps are inputs read from `Global`, not hardcoded. |
| `imperial-perps`, `phoenix-perps` | isolated-margin liquidation price and margin | `IMPERIAL_TRADING_ENABLED=true` is refused. `IMPERIAL_JWT` is never read. Venue program ids are not hardcoded (unverified). |
| `ore-mining` | pro-rata expected reward from round state | No hashing, no deploys. Program id not hardcoded. |

Tests: `tests/modules.rs` (5). They check that every module's `execute` errors, that the Imperial flag is refused, and the quote, liquidation and share math.

Next: scoped execution through `san_delegation` (PR #3) once a module is reviewed for a non-simulated mode. That is not planned for mainnet without explicit owner approval.
