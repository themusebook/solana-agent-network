//! `solana-agent`: the SAN agent definition (phase 1, devnet only).
//! A registration document that is ERC-8004 compatible, readable alongside the Metaplex
//! 014 agent registry (mpl-agent), x402-ready and A2A-ready by default, plus scoped,
//! capped and expiring delegations. The server never signs; owners sign.
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const AGENT_NAME: &str = "solana-agent";
pub const MEMO_PROGRAM: &str = "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr";
pub const MPL_AGENT_IDENTITY: &str = "1DREGFgysWYxLnRnKQnwrxnJQeSMk2HmGaC6whw2B2p";
pub const MPL_AGENT_TOOLS: &str = "TLREGni9ZEyGC3vnPZtqUh95xQ8oPqJSvNjvB7FGK8S";

/// Capability modules. Trading modules ship disabled/simulated in phase 1.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Module { pub id: String, pub kind: String, pub enabled: bool, pub mode: String }

pub fn default_modules() -> Vec<Module> {
    let m = |id: &str, kind: &str, en: bool, mode: &str| Module { id: id.into(), kind: kind.into(), enabled: en, mode: mode.into() };
    vec![
        m("x402m-messaging", "messaging", true, "live"),
        m("x402-payments", "payments", true, "devnet-proposal-only"),
        m("ore-mining", "adapter", false, "simulated"),
        m("imperial-perps", "adapter", false, "simulated"),
        m("phoenix-perps", "adapter", false, "simulated"),
        m("pump-fun", "adapter", false, "simulated"),
        m("helius-core-ai", "skills", false, "planned"),
    ]
}

pub struct Input<'a> { pub asset: &'a str, pub wallet: &'a str, pub endpoint: &'a str, pub version: u32 }

/// ERC-8004 registration-v1 document with x402 + A2A sections. Hosted at
/// `{endpoint}/agents/{asset}/registration.json`; its sha256 is anchored on-chain.
pub fn registration(i: &Input) -> Value {
    let base = format!("{}/agents/{}", i.endpoint.trim_end_matches('/'), i.asset);
    json!({
        "type": "https://eips.ethereum.org/EIPS/eip-8004#registration-v1",
        "name": AGENT_NAME,
        "description": "Solana Agent Network agent (devnet)",
        "version": i.version,
        "services": [
            {"name": "A2A", "endpoint": format!("{base}/.well-known/agent-card.json"), "version": "0.3.0"},
            {"name": "x402m", "endpoint": format!("{base}/x402m")},
            {"name": "MCP", "endpoint": format!("{}/mcp", i.endpoint.trim_end_matches('/'))}
        ],
        "x402": {"network": "solana-devnet", "payTo": i.wallet, "endpoint": format!("{base}/x402")},
        "registrations": [{"agentId": i.asset, "agentRegistry": format!("solana:devnet:{MPL_AGENT_IDENTITY}")}],
        "supportedTrust": ["reputation"],
        "modules": default_modules(),
    })
}

/// Minimal A2A agent card.
pub fn a2a_card(i: &Input) -> Value {
    let skills: Vec<Value> = default_modules().into_iter().filter(|m| m.enabled)
        .map(|m| json!({"id": m.id, "name": m.id, "tags": [m.kind]})).collect();
    json!({"name": AGENT_NAME, "url": format!("{}/agents/{}/a2a", i.endpoint.trim_end_matches('/'), i.asset),
           "version": i.version.to_string(), "capabilities": {"streaming": false},
           "defaultInputModes": ["application/json"], "defaultOutputModes": ["application/json"], "skills": skills})
}

/// Canonical (sorted-key) JSON bytes and their sha256 hex.
pub fn content_hash(doc: &Value) -> String {
    let bytes = serde_json::to_vec(doc).expect("json"); // serde_json Map is BTreeMap => sorted
    hex(&Sha256::digest(bytes))
}
fn hex(b: &[u8]) -> String { b.iter().map(|x| format!("{x:02x}")).collect() }

/// Memo anchor body. Updatable: a higher `version` supersedes the previous anchor
/// (fixes "one-time registration, fixed URI"). Cost measured on devnet: 5,000 lamports, 0 rent.
pub fn anchor_memo(asset: &str, version: u32, hash: &str) -> String { format!("san1:reg:{asset}:{version}:{hash}") }

pub fn parse_anchor(m: &str) -> Result<(String, u32, String)> {
    let p: Vec<&str> = m.split(':').collect();
    if p.len() != 5 || p[0] != "san1" || p[1] != "reg" || p[4].len() != 64 { bail!("not a san1 anchor") }
    Ok((p[2].into(), p[3].parse()?, p[4].into()))
}

/// Scoped delegation (fixes "arbitrary, unscoped, survives transfer, no expiry").
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Delegation {
    pub asset: String, pub delegate: String,
    /// Owner at grant time; any transfer invalidates the delegation.
    pub owner_at_grant: String,
    pub programs: Vec<String>, pub spend_cap_lamports: u64, pub spent_lamports: u64,
    pub expires_at: i64, pub nonce: u64,
}

impl Delegation {
    /// `revoked_nonce`: owner bumps a single counter to revoke every delegation at once,
    /// so downstream approvals derived from an older nonce stop working too.
    pub fn check(&self, current_owner: &str, program: &str, lamports: u64, now: i64, revoked_nonce: u64) -> Result<()> {
        if current_owner != self.owner_at_grant { bail!("asset transferred: delegation void") }
        if self.nonce < revoked_nonce { bail!("revoked") }
        if now >= self.expires_at { bail!("expired") }
        if !self.programs.iter().any(|p| p == program) { bail!("program not allowlisted") }
        if self.spent_lamports.saturating_add(lamports) > self.spend_cap_lamports { bail!("spend cap exceeded") }
        Ok(())
    }
}
