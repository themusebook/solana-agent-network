//! ORE mining (simulated). Expected-reward estimate from caller-supplied round state; no hashing
//! submissions, no deploys, no signing. Program id not hardcoded (supply via intent if needed).
use super::{f, Module};
use anyhow::{bail, Result};
use serde_json::{json, Value};
pub struct Ore;
/// Share of a round's reward for deploying `stake` into a square holding `square_total` (pro-rata).
pub fn expected_share(stake: f64, square_total: f64, round_reward: f64, p_win: f64) -> Result<f64> {
    if stake <= 0.0 || square_total < 0.0 || !(0.0..=1.0).contains(&p_win) { bail!("bad inputs") }
    Ok(p_win * round_reward * stake / (square_total + stake))
}
impl Module for Ore {
    fn id(&self) -> &'static str { "ore-mining" }
    fn describe(&self) -> Value { json!({"id": self.id(), "mode": "simulated"}) }
    fn simulate(&self, i: &Value) -> Result<Value> {
        let e = expected_share(f(i,"stake")?, f(i,"squareTotal")?, f(i,"roundReward")?, f(i,"pWin")?)?;
        Ok(json!({"simulated": true, "expectedReward": e, "stake": f(i,"stake")?}))
    }
}
