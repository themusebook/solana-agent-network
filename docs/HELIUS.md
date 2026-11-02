# Helius core-ai skills and fee routing (devnet)

- `skills/helius/{build,phantom,jupiter,dflow,okx,svm}` are vendored unmodified from helius-labs/core-ai under the MIT License. See `skills/helius/README.md`.
- `src/helius.rs`:
  - The priority fee is always fetched live, never hardcoded. With `HELIUS_API_KEY` set it uses Helius `getPriorityFeeEstimate` (High); otherwise it takes the 75th percentile of `getRecentPrioritizationFees`. It also builds the ComputeBudget instruction data.
  - Helius Sender (`sender.helius-rpc.com/fast`) is declared but disabled, because Sender is mainnet-only and this build is devnet-only. The tip will come from `SAN_SENDER_TIP_LAMPORTS`, with no default.
  - Keys are read from the environment only and never logged. `JUPITER_API_KEY` and `DFLOW_API_KEY` are optional; the skills that need them report `requiresKey`.
- Capabilities are declared in the registration document (`capabilities`) and the A2A card (`skills`). This changes the content hash; the new test vector is `b93214e7…`.
- Live check (2026-11-02, public devnet RPC, no Helius key): `rpc:getRecentPrioritizationFees:p75 = 0` micro-lamports/CU on the Memo program.
