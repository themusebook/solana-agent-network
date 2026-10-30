//! Network card (ported from bridge/network-card.mjs): a PUBLIC binding of agent id,
//! Solana wallet and (optionally) the joined Metaplex Core asset. Never carries secrets.
use crate::config::{is_pubkey, NETWORK};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const SECRET_FIELDS: &[&str] = &[
    "walletSecret", "privateKey", "secretKey", "seed", "seedPhrase", "mnemonic",
    "agentSecret", "agentPrivateKey", "sessionToken", "token", "apiKey", "proof",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NetworkCard {
    pub san_card: String,
    pub agent_id: String,
    pub wallet: String,
    #[serde(skip_serializing_if = "Option::is_none")] pub asset: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub handle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub name: Option<String>,
    pub network: String,
    pub transports: Vec<String>,
    pub endpoints: BTreeMap<String, String>,
}

#[derive(Default)]
pub struct CardInput<'a> {
    pub agent_id: &'a str, pub wallet: &'a str, pub asset: Option<&'a str>,
    pub handle: Option<&'a str>, pub name: Option<&'a str>, pub mailbox: Option<&'a str>,
}

pub fn build(i: CardInput, endpoint: &str) -> Result<NetworkCard> {
    if i.agent_id.trim().is_empty() { bail!("network card requires agentId"); }
    if !is_pubkey(i.wallet) { bail!("network card requires a valid Solana wallet"); }
    if let Some(a) = i.asset { if !is_pubkey(a) { bail!("asset must be a base58 address"); } }
    let handle = i.handle.map(normalize_handle).transpose()?;
    let mut endpoints = BTreeMap::new();
    endpoints.insert("mailbox".into(), i.mailbox.map(str::to_string).unwrap_or_else(|| format!("{endpoint}/api/san/mailbox")));
    endpoints.insert("discovery".into(), format!("{endpoint}/api/network/agents"));
    Ok(NetworkCard {
        san_card: "v1".into(), agent_id: i.agent_id.into(), wallet: i.wallet.into(),
        asset: i.asset.map(Into::into), handle, name: i.name.map(Into::into),
        network: NETWORK.into(), transports: vec!["https-mailbox".into(), "x402m".into()], endpoints,
    })
}

pub fn normalize_handle(h: &str) -> Result<String> {
    let n = h.trim_start_matches('@').to_ascii_lowercase();
    let ok = n.len() >= 3 && n.len() <= 32 && n.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && n.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
    if !ok { bail!("invalid handle"); }
    Ok(n)
}

/// Reject any JSON card that carries a secret-looking field (recursively).
pub fn reject_secrets(v: &Value) -> Result<()> {
    match v {
        Value::Object(m) => {
            for (k, val) in m {
                if SECRET_FIELDS.iter().any(|s| s.eq_ignore_ascii_case(k)) { bail!("card must not contain secret field `{k}`"); }
                reject_secrets(val)?;
            }
        }
        Value::Array(a) => for x in a { reject_secrets(x)?; },
        _ => {}
    }
    Ok(())
}

/// Parse an untrusted card: secrets rejected, wallet validated, devnet only.
pub fn parse(v: &Value) -> Result<NetworkCard> {
    reject_secrets(v)?;
    let c: NetworkCard = serde_json::from_value(v.clone())?;
    if !is_pubkey(&c.wallet) { bail!("invalid wallet"); }
    if c.network != NETWORK { bail!("only {NETWORK} cards are accepted"); }
    Ok(c)
}

/// Address book keys: @handle, agentId, asset, wallet.
pub fn address_keys(c: &NetworkCard) -> Vec<String> {
    let mut k = vec![c.agent_id.clone(), c.wallet.clone()];
    if let Some(h) = &c.handle { k.push(format!("@{h}")); }
    if let Some(a) = &c.asset { k.push(a.clone()); }
    k
}
