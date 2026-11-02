//! Imperial and Phoenix perps (simulated). Trading is hard-disabled: IMPERIAL_TRADING_ENABLED must be
//! "false" (or unset). IMPERIAL_JWT is never read, logged, or forwarded by this build.
//! Program ids are not hardcoded (unverified in this PR); simulation is venue-agnostic margin math.
use super::{f, Module};
use anyhow::{bail, Result};
use serde_json::{json, Value};
pub struct Perps { venue: &'static str }
impl Perps { pub fn imperial() -> Self { Perps { venue: "imperial-perps" } } pub fn phoenix() -> Self { Perps { venue: "phoenix-perps" } } }

pub fn trading_enabled() -> bool { std::env::var("IMPERIAL_TRADING_ENABLED").map(|v| v == "true").unwrap_or(false) }

/// Isolated-margin liquidation price (linear perp). maint = maintenance margin fraction.
pub fn liq_price(entry: f64, leverage: f64, maint: f64, long: bool) -> Result<f64> {
    if entry <= 0.0 || leverage < 1.0 || !(0.0..1.0).contains(&maint) { bail!("bad inputs") }
    let d = 1.0 / leverage - maint;
    Ok(if long { entry * (1.0 - d) } else { entry * (1.0 + d) })
}

impl Module for Perps {
    fn id(&self) -> &'static str { self.venue }
    fn describe(&self) -> Value { json!({"id": self.venue, "mode": "simulated", "tradingEnabled": false}) }
    fn simulate(&self, i: &Value) -> Result<Value> {
        let long = i["side"].as_str() == Some("long");
        let (entry, lev, size) = (f(i,"entryPrice")?, f(i,"leverage")?, f(i,"sizeUsd")?);
        let liq = liq_price(entry, lev, f(i,"maintenanceMargin")?, long)?;
        Ok(json!({"simulated": true, "venue": self.venue, "side": if long {"long"} else {"short"}, "marginUsd": size / lev, "liquidationPrice": liq}))
    }
    fn execute(&self, _: &Value) -> Result<Value> {
        if trading_enabled() { bail!("{}: IMPERIAL_TRADING_ENABLED=true is refused in this devnet build", self.venue) }
        bail!("{}: execution disabled (simulation only)", self.venue)
    }
}
