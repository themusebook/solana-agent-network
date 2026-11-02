# solana-agent (phase 1, devnet)

`solana-agent` is the SAN agent definition: an ERC-8004 registration document that also
lists the agent in the Metaplex 014 registry (`1DREGFgy…`), x402-ready and A2A-ready by
default, with messaging on x402m/1. We aim for it to be one of the most complete open
Solana agent definitions. Devnet only. Nothing here signs: owners sign in their own wallet.

## Measured costs (devnet, 2026-11-02, `bench/bench.mjs`)
Rent on devnet and mainnet is currently 5,080 lamports/byte (rent-exempt minimum for 0 bytes = 650,240 on both).

| Step | Lamports | Signature |
|---|---|---|
| Metaplex: createCollection (98 B, rent 1,148,080) | 1,158,080 | 3fiovA2q1ep6Qiu3nfjBwnXg1SUbXNPEbJkkNGYWG2BcjkAKqCrgfRqEhraCGmmHtC26heetvXP6JYkRuKQPDpBG |
| Metaplex: Core create asset (122 B, includes the Core fee held in the asset) | 2,780,000 | 2d6Z81mT8CWPLgvk3wxWCXg9Av6fxy2eBTC7eqTajXFT1JXYZjCWSzGkaH6A45tpx7HvCWxfeK6UgCA9J15GbnZq |
| Metaplex: registerIdentityV1 (identity PDA 1,178,560 + asset realloc 482,600 + fee) | 1,666,160 | 22q9hSaDFyLG5xstVFfFMacXHouMLpWatZnGcnW8vsZSvHMtgnwkYwMtCG9v3fFQS4dzH5E4h1JyjYgCHorEWZzY |
| SAN: content-hash memo anchor (0 rent) | 5,000 | 4GM3TWhqLRzbmnAUyhQZnTN4czur1uriaLTeD6E6svXxdtwSFA2g7wME3N8M5vuCSPPg8t5YjwkiXLg7WHtmDbrZ |

Per agent, Arweave excluded:
- 014 baseline, shared collection: asset + identity = **4,446,160** (first agent with its own collection: 5,604,240).
- SAN anchor-only (no DAS asset): **5,000**.
- SAN with a DAS-visible Core asset in a shared collection + anchor: **2,785,000**. That is 37% less than the baseline, and it skips the identity PDA and the Arweave upload.

The SAN on-chain node record (`register_node`, 250 B) measured at 1,925,240 lamports. It is optional in this model.

## What it fixes in the 014 flow
- **Updatable URI:** `san1:reg:<asset>:<version>:<sha256>` memos. The highest version wins, and the document is hosted on registry.musebook.trade (or AO) with its hash checked on-chain.
- **Scoped delegations** (`agent::Delegation`): a program allowlist, a spend cap, expiry, voided when the asset is transferred, and a single revocation nonce that also voids downstream approvals.
- **No per-agent rent** on the anchor-only path. Nothing is permanent the way `agent_token` is.

## Status
Phase 1 is the SDK and spec, with tests. Not yet done: an on-chain program that enforces delegations (today the SDK checks them, the chain does not); registry endpoints `/agents/<asset>/registration.json` and an A2A card (that needs a PR in agent-network); the ORE, Imperial, Phoenix and pump.fun modules (disabled or simulated); and the Helius core-ai skills.
