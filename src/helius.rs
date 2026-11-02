//! Fee routing for solana-agent (devnet). Keys come from the environment only and are never logged.
//! - Priority fee: Helius `getPriorityFeeEstimate` when HELIUS_API_KEY is set, else the standard
//!   `getRecentPrioritizationFees` (75th percentile). Never hardcoded.
//! - Helius Sender (https://sender.helius-rpc.com/fast) is mainnet-only, so it is gated off here;
//!   the route is planned and the tip comes from SAN_SENDER_TIP_LAMPORTS (no default).
use crate::config::guard_devnet;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

pub const SENDER_URL: &str = "https://sender.helius-rpc.com/fast";

#[derive(Debug, Clone, PartialEq)]
pub enum Route { DevnetRpc { rpc: String }, Sender { url: String, tip_lamports: u64 } }

pub fn helius_devnet_rpc() -> Option<String> {
    std::env::var("HELIUS_API_KEY").ok().filter(|k| !k.is_empty())
        .map(|k| format!("https://devnet.helius-rpc.com/?api-key={k}"))
}

/// Pick the submit route. `mainnet` is always refused in this build.
pub fn route(rpc: &str, mainnet: bool) -> Result<Route> {
    if mainnet { bail!("Helius Sender is mainnet-only; solana-agent runs on devnet only") }
    guard_devnet(rpc)?;
    Ok(Route::DevnetRpc { rpc: rpc.into() })
}

/// p-th percentile of `prioritizationFee` values (micro-lamports per CU).
pub fn percentile(mut fees: Vec<u64>, p: f64) -> u64 {
    if fees.is_empty() { return 0 }
    fees.sort_unstable();
    fees[(((fees.len() - 1) as f64) * p).round() as usize]
}

/// Live priority-fee estimate in micro-lamports per CU for transactions touching `accounts`.
pub fn priority_fee(rpc: &str, accounts: &[&str]) -> Result<(u64, &'static str)> {
    if let Some(h) = helius_devnet_rpc() {
        let b = json!({"jsonrpc":"2.0","id":1,"method":"getPriorityFeeEstimate",
            "params":[{"accountKeys": accounts, "options": {"priorityLevel": "High"}}]});
        let v: Value = ureq::post(&h).send_json(b).context("helius request failed")?.into_json()?;
        let f = v.pointer("/result/priorityFeeEstimate").and_then(Value::as_f64).context("no estimate")?;
        return Ok((f.ceil() as u64, "helius:getPriorityFeeEstimate:High"));
    }
    guard_devnet(rpc)?;
    let v: Value = ureq::post(rpc).send_json(json!({"jsonrpc":"2.0","id":1,"method":"getRecentPrioritizationFees","params":[accounts]}))?.into_json()?;
    let fees = v["result"].as_array().map(|a| a.iter().filter_map(|x| x["prioritizationFee"].as_u64()).collect()).unwrap_or_default();
    Ok((percentile(fees, 0.75), "rpc:getRecentPrioritizationFees:p75"))
}

/// ComputeBudget SetComputeUnitPrice instruction data: [3, u64 LE].
pub fn set_cu_price_data(micro_lamports: u64) -> Vec<u8> { let mut d = vec![3u8]; d.extend_from_slice(&micro_lamports.to_le_bytes()); d }
/// ComputeBudget SetComputeUnitLimit instruction data: [2, u32 LE].
pub fn set_cu_limit_data(units: u32) -> Vec<u8> { let mut d = vec![2u8]; d.extend_from_slice(&units.to_le_bytes()); d }

/// Capabilities to declare in the registration doc / A2A card.
pub fn capabilities() -> Value {
    let skills: Vec<Value> = ["build","phantom","jupiter","dflow","okx","svm"].iter().map(|s| json!({
        "id": format!("helius-{s}"), "source": "github.com/helius-labs/core-ai", "license": "MIT",
        "requiresKey": match *s { "jupiter" => Some("JUPITER_API_KEY"), "dflow" => Some("DFLOW_API_KEY"), "build"|"svm"|"phantom"|"okx" => Some("HELIUS_API_KEY"), _ => None },
    })).collect();
    json!({"skills": skills, "feeRouting": {"priorityFee": "dynamic", "sender": {"url": SENDER_URL, "enabled": false, "reason": "mainnet-only"}}})
}
