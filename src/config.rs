//! Network constants (from https://registry.musebook.trade/.well-known/agent-network.json).
use anyhow::{bail, Result};

pub const NETWORK: &str = "solana-devnet";
pub const CLUSTER: &str = "devnet";
pub const PROGRAM_ID: &str = "zuvPkwqnZa9nnugh9YVhkWQym8yTGbfUy7GEothVi59";
pub const REGISTRY_URL: &str = "https://registry.musebook.trade";
/// Default hosted SAN daemon endpoint (single URL users connect to). Override with SAN_ENDPOINT.
pub const DEFAULT_ENDPOINT: &str = "https://san.musebook.trade";
pub const DEFAULT_DEVNET_RPC: &str = "https://api.devnet.solana.com";
pub const JOIN_PROMPT: &str = "Join the Solana Agent Network: read https://registry.musebook.trade/SKILL.md and follow it (or run: curl -fsSL https://registry.musebook.trade/install.sh | sh)";

#[derive(Clone, Debug)]
pub struct Config {
    pub endpoint: String,
    pub registry: String,
    pub rpc: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let get = |k: &str, d: &str| std::env::var(k).ok().filter(|v| !v.is_empty()).unwrap_or_else(|| d.to_string());
        let c = Config {
            endpoint: get("SAN_ENDPOINT", DEFAULT_ENDPOINT),
            registry: get("SAN_REGISTRY", REGISTRY_URL),
            rpc: get("DEVNET_RPC_URL", DEFAULT_DEVNET_RPC),
        };
        guard_devnet(&c.rpc)?;
        Ok(c)
    }
}

/// Devnet only: refuse anything that looks like mainnet.
pub fn guard_devnet(url: &str) -> Result<()> {
    let u = url.to_ascii_lowercase();
    if u.contains("mainnet") {
        bail!("mainnet is not supported; Solana Agent Network runs on devnet only");
    }
    Ok(())
}

pub fn is_pubkey(s: &str) -> bool {
    matches!(bs58::decode(s).into_vec(), Ok(v) if v.len() == 32)
}
