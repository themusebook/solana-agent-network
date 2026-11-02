# san_delegation (devnet)

Program id (devnet): `EeZBH7PggMHuQoYjK9Lz6bsM7GvbmKk9pBn9dDxV57Tm`. Deploy signature: `rJogLqMCHWhAaYn1NHtdZE2F24LvszvMueJL2ecpSHtcFungRL6o22ni1DqLupi5G5EN2QAQZ5i2Je5RuGNWACN`.

This is original code, not copied from mpl-agent (mpl-agent uses the Metaplex NFT license). It is checked against mpl-agent-tools `ExecutionDelegateRecordV1`, which is 104 bytes, seeded `[execution_delegate_record, executive_profile, asset]`, and holds only key, bump, executive_profile, authority and agent_asset, with no scope.

| | mpl-agent-tools | san_delegation |
|---|---|---|
| Program scope | any | allowlist of up to 4 programs |
| Spend | unlimited | lamport cap, tracked in the record |
| Expiry | none | `expires_at` |
| Asset transfer | delegate survives | void (`owner_at_grant` must equal the current Core owner) |
| Revoke | per record | `revoke_all` bumps an owner-wide nonce, voiding every delegation at once |
| Rent | 104-byte record | 266-byte record + 49-byte owner state (first grant only), reclaimable via `close` |

Instructions: `grant`, `consume` (the delegate records use; it fails unless every scope check passes), `revoke_all` and `close`.

## Devnet e2e (`bench/delegation-e2e.mjs`, 2026-11-02)
- Core asset `6TMsXVCC3w1pNafAkF4KYJiB7MFYeBZtpvJ3jhSXnjWC`, created in `4DndymsD…gAdhj`.
- `grant`, 2,905,680 lamports including fee: `4PVUhby83NQRZ7DVxTUusR8D4N4BxAwhRD4WV8nP7Dg6rhu9cwv6qsGagwwo51r5Y3sbLNhjetwkMBfzVctDFfhA`
- `consume` 600 of 1000: `c7bEmwA5jbjSbfCqQkdY4dxsyndvScMLEk58oPbRARPBVaXrzxccsfQ9CiZQqN85DTFgcTHJ8XNpxwEAQTtyBhY`
- A second 600 was rejected with `CapExceeded`. The System program was rejected with `NotAllowlisted`.
- Asset transfer: `5nMxdWJ2ZH9hrGg1Y6Thpi2WuecB9P9C8VBxKkMGD47SdKhJvLZPRKXpQpBsLuqDNdRkf78s1GF5izt7LNaub3NJ`. `consume` was then rejected with `Transferred`.
- `close` by the delegate reclaimed the rent: `4Yc5sxbcG2niuAWmsyov8jJrwuiRQACTJnK5JCqGQ1q834htfkZNfbaXWZNBTG3ktnERVtEZYhCXhRZC5geaZxQh`

## revoke_all e2e (2026-11-02, asset `EEkvCt6wMwNogCDiHuUyEru9WGeFmwtDfJFpzf2mmZha`)
- `grant`: `fENm5bPngyKzonKaCvcZt3hogqqXFoSgda3ziKHAPQxzqTidZjLWW4iPd5v4PJ8rp47rAowfhPGPhKFsqXwaGMP`
- `consume` 600: `5xijKScG1wUyWM1V75YUHb7mW58Cb3G8WkYRZNZvVD1g2JwaXMjncMhugPP2VeVE4qmBuzXEZsuDaG4itPdCmW9H`
- `revoke_all`: `45nMfYepJxyFWyPLSWaDGcBHJANHv6YwErrzeD5Dh1i2M3KboVmcQo2gMVoKcuhyU922UdN8Txp7v4pLtaiTeW6D`. The next `consume` was rejected with `Revoked`.
- The owner closed the revoked delegation: `48KPHdhx1id9zJtQ128fxzHY7t5bqkj8WZonCfXq6vJGiqo9wapatfVDaaWjHAZQVLXrnMsLXoG5EdEjJ5ziKtnd`
- Re-grant at the new nonce: `34uLadATmQnmrbgf9SyAyVNwPLDoZ2wW2McqMwR7VX5XPsMzZNSYtZzRoJTgbTxw8hn1RZvmrKy33Kap8uvD75Ab`. The following `consume` succeeded: `5huh6U4V5Tx8FJvhNJ7fv2JoGE3GH9EeHdeYrNewtQ34NHizYeQ9X7eeGVYa18tgVX6JvWUR9TArMUv2b9ii7W98`
- Over the cap and off the allowlist were rejected again. The consume after the transfer (`38BnfTQ3…`) was rejected.
- Delegate close: `2TVd5cVQ…`. Sweeps: `4xZCgyEN…` and `5G8BXinW…`.

There is no audit. Devnet only.
