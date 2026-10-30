//! HTTP client for the SAN registry / hosted daemon (same /api/network/* surface).
use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Clone)]
pub struct Registry {
    pub base: String,
    agent: ureq::Agent,
}

impl Registry {
    pub fn new(base: &str) -> Self {
        let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(20)).user_agent("solana-agent-network/0.1").build();
        Registry { base: base.trim_end_matches('/').to_string(), agent }
    }
    fn get(&self, path: &str) -> Result<Value> {
        let r = self.agent.get(&format!("{}{}", self.base, path)).call();
        decode(r)
    }
    fn post(&self, path: &str, body: Value) -> Result<Value> {
        decode(self.agent.post(&format!("{}{}", self.base, path)).send_json(body))
    }
    pub fn health(&self) -> Result<Value> { self.get("/api/network/health") }
    pub fn stats(&self) -> Result<Value> { self.get("/api/network/stats") }
    pub fn manifest(&self) -> Result<Value> { self.get("/.well-known/agent-network.json") }
    pub fn agents(&self) -> Result<Value> { self.get("/api/network/agents") }
    pub fn agent(&self, asset: &str) -> Result<Value> { check(asset)?; self.get(&format!("/api/network/agents/{asset}")) }
    pub fn verify(&self, asset: &str) -> Result<Value> { check(asset)?; self.get(&format!("/api/network/verify/{asset}")) }
    pub fn invite_check(&self, link: &str) -> Result<Value> { self.post("/api/network/invites/check", json!({"link": link})) }
    /// Returns an UNSIGNED register_node transaction for the OWNER's wallet to sign. Never signed here.
    pub fn register_node_tx(&self, asset: &str, authority: &str) -> Result<Value> {
        check(asset)?;
        check(authority)?;
        self.post("/api/network/tx/register-node", json!({"asset": asset, "authority": authority}))
    }
    pub fn join_url(&self) -> String { format!("{}/network/join", self.base) }
}

fn check(k: &str) -> Result<()> {
    if !crate::config::is_pubkey(k) { bail!("not a base58 Solana address: {k}") }
    Ok(())
}

fn decode(r: Result<ureq::Response, ureq::Error>) -> Result<Value> {
    match r {
        Ok(resp) => Ok(resp.into_json()?),
        Err(ureq::Error::Status(code, resp)) => {
            let v: Value = resp.into_json().unwrap_or(json!({}));
            Err(anyhow!("registry error {code}: {v}"))
        }
        Err(e) => Err(anyhow!(e)),
    }
}

/// Try primary (hosted daemon) then fallback (registry).
pub struct Upstream { pub primary: Registry, pub fallback: Registry }

impl Upstream {
    pub fn new(primary: &str, fallback: &str) -> Self { Upstream { primary: Registry::new(primary), fallback: Registry::new(fallback) } }
    pub fn call<T>(&self, f: impl Fn(&Registry) -> Result<T>) -> Result<T> {
        f(&self.primary).or_else(|_| f(&self.fallback))
    }
}
