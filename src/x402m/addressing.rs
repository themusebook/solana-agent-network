//! Recipient resolution (port of solana/addressing.mjs). Never guesses a wallet.
use crate::card::NetworkCard;
use anyhow::{bail, Result};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Resolved { pub wallet: String, pub agent_id: Option<String>, pub handle: Option<String>, pub source: String }

pub fn address_book(cards: &[NetworkCard]) -> HashMap<String, NetworkCard> {
    let mut m = HashMap::new();
    for c in cards {
        m.insert(c.agent_id.clone(), c.clone());
        m.insert(c.wallet.clone(), c.clone());
        if let Some(h) = &c.handle { m.insert(h.to_ascii_lowercase(), c.clone()); }
        if let Some(r) = &c.reason { m.insert(r.to_ascii_lowercase(), c.clone()); }
    }
    m
}

/// `directory` = musebook /api/x402m/agents `agents` array ([{agent_id, handle, ...}]).
pub fn resolve(input: &str, cards: &[NetworkCard], directory: &[Value]) -> Result<Resolved> {
    if input.trim().is_empty() { bail!("recipient required"); }
    if crate::config::is_pubkey(input) { return Ok(Resolved { wallet: input.into(), agent_id: None, handle: None, source: "wallet".into() }); }
    let key = input.trim_start_matches('@').to_ascii_lowercase();
    let book = address_book(cards);
    if let Some(c) = book.get(&key).or_else(|| book.get(input)) {
        return Ok(Resolved { wallet: c.wallet.clone(), agent_id: Some(c.agent_id.clone()), handle: c.handle.clone(), source: "address-book".into() });
    }
    let known = directory.iter().find(|a| a["agent_id"].as_str() == Some(input) || a["handle"].as_str().map(|h| h.to_ascii_lowercase()) == Some(key.clone()));
    if let Some(a) = known {
        bail!("{input} is a registered messaging identity ({}) but no verified network card binds it to a wallet. Ask the agent to publish its card or add its wallet to your address book.", a["agent_id"].as_str().unwrap_or("?"));
    }
    bail!("cannot resolve recipient: {input}")
}
