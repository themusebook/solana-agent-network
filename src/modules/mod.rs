//! Capability modules for solana-agent. Phase 4: SIMULATED ONLY.
//! No module signs, sends, or trades. `execute` always refuses. Users sign; the server never signs.
use anyhow::{bail, Result};
use serde_json::Value;
pub mod ore; pub mod perps; pub mod pumpfun;

pub trait Module {
    fn id(&self) -> &'static str;
    fn describe(&self) -> Value;
    /// Pure simulation from caller-supplied state: returns a plan/quote, never a signed tx.
    fn simulate(&self, intent: &Value) -> Result<Value>;
    fn execute(&self, _intent: &Value) -> Result<Value> { bail!("{}: execution disabled (simulation only, devnet build)", self.id()) }
}

pub fn all() -> Vec<Box<dyn Module>> { vec![Box::new(ore::Ore), Box::new(perps::Perps::imperial()), Box::new(perps::Perps::phoenix()), Box::new(pumpfun::PumpFun)] }

pub(crate) fn u(v: &Value, k: &str) -> Result<u64> { v[k].as_u64().ok_or_else(|| anyhow::anyhow!("missing u64 `{k}`")) }
pub(crate) fn f(v: &Value, k: &str) -> Result<f64> { v[k].as_f64().ok_or_else(|| anyhow::anyhow!("missing number `{k}`")) }
