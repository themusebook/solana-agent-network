//! pump.fun bonding-curve quote (simulated). Program ids and formula from pump-fun/pump-public-docs
//! (PUMP_PROGRAM_README.md: constant-product over virtual reserves; fee_basis_points from Global).
//! Fees are inputs read from Global by the caller; nothing is hardcoded. No signing, no trading.
use super::{u, Module};
use anyhow::{bail, Result};
use serde_json::{json, Value};
pub const PUMP_PROGRAM: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
pub const PUMP_AMM_PROGRAM: &str = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";
pub const PUMP_FEES_PROGRAM: &str = "pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ";
pub struct PumpFun;

/// tokens_out for `sol_in` lamports: fee taken first, then x*y=k over virtual reserves.
pub fn quote_buy(v_sol: u64, v_tok: u64, real_tok: u64, sol_in: u64, fee_bps: u64, creator_fee_bps: u64) -> Result<(u64, u64)> {
    if v_sol == 0 || v_tok == 0 { bail!("empty curve") }
    let fee = (sol_in as u128 * (fee_bps + creator_fee_bps) as u128).div_ceil(10_000) as u64;
    let net = sol_in.checked_sub(fee).ok_or_else(|| anyhow::anyhow!("fee exceeds input"))?;
    let k = v_sol as u128 * v_tok as u128;
    let out = v_tok as u128 - k.div_ceil(v_sol as u128 + net as u128);
    Ok(((out as u64).min(real_tok), fee))
}

impl Module for PumpFun {
    fn id(&self) -> &'static str { "pump-fun" }
    fn describe(&self) -> Value { json!({"id": self.id(), "mode": "simulated", "programs": [PUMP_PROGRAM, PUMP_AMM_PROGRAM, PUMP_FEES_PROGRAM], "reference": "github.com/pump-fun/pump-public-docs"}) }
    fn simulate(&self, i: &Value) -> Result<Value> {
        let (out, fee) = quote_buy(u(i,"virtualSolReserves")?, u(i,"virtualTokenReserves")?, u(i,"realTokenReserves")?, u(i,"solIn")?, u(i,"feeBasisPoints")?, i["creatorFeeBasisPoints"].as_u64().unwrap_or(0))?;
        Ok(json!({"simulated": true, "side": "buy", "tokensOut": out, "feeLamports": fee}))
    }
}
