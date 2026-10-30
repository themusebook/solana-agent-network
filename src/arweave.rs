//! Arweave + AO: archive network cards permanently and discover them via an AR.IO gateway.
//! Defaults: gateway https://io.musebook.trade (AR.IO), AO https://ao.musebook.trade.
//! Uploads are built UNSIGNED for the OWNER to sign (ANS-104 data item, Solana ed25519 = signature type 2)
//! and post to Turbo. This crate never holds keys and never pays for uploads.
use crate::card::{self, NetworkCard};
use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};

pub const DEFAULT_GATEWAY: &str = "https://io.musebook.trade";
pub const DEFAULT_AO: &str = "https://ao.musebook.trade";
pub const TURBO_UPLOAD: &str = "https://upload.ardrive.io/v1/tx";
pub const APP_NAME: &str = "Solana-Agent-Network";
pub const FREE_LIMIT: usize = 100 * 1024;

pub fn gateway() -> String { env_or("ARWEAVE_GATEWAY", DEFAULT_GATEWAY) }
pub fn ao_url() -> String { env_or("AO_URL", DEFAULT_AO) }
fn env_or(k: &str, d: &str) -> String {
    std::env::var(k).ok().filter(|v| !v.is_empty()).unwrap_or_else(|| d.into()).trim_end_matches('/').into()
}

pub fn is_txid(s: &str) -> bool {
    s.len() == 43 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn card_tags(c: &NetworkCard) -> Vec<(String, String)> {
    let mut t = vec![
        ("Content-Type".into(), "application/json".into()),
        ("App-Name".into(), APP_NAME.into()),
        ("App-Version".into(), env!("CARGO_PKG_VERSION").into()),
        ("Type".into(), "network-card".into()),
        ("Network".into(), c.network.clone()),
        ("Wallet".into(), c.wallet.clone()),
        ("Agent-Id".into(), c.agent_id.clone()),
    ];
    if let Some(a) = &c.asset { t.push(("Asset".into(), a.clone())); }
    t
}

/// Unsigned upload payload: data + tags + instructions. The owner signs and posts it.
pub fn archive_payload(c: &NetworkCard) -> Result<Value> {
    let v = serde_json::to_value(c)?;
    card::reject_secrets(&v)?;
    let data = serde_json::to_string(&v)?;
    if data.len() > FREE_LIMIT { bail!("card is {} bytes; keep it under 100 KB (free on Turbo)", data.len()); }
    let tags: Vec<Value> = card_tags(c).into_iter().map(|(n, v)| json!({"name":n,"value":v})).collect();
    Ok(json!({
        "format":"ANS-104 data item (unsigned)","signatureType":2,"signer":"owner Solana wallet (ed25519)",
        "upload":{"method":"POST","url":TURBO_UPLOAD,"contentType":"application/octet-stream"},
        "bytes":data.len(),"freeTier":true,"tags":tags,"data":data,
        "instructions":"Sign this data item with the OWNER's wallet (e.g. @ardrive/turbo-sdk TurboFactory.authenticated with the owner's Solana signer in their own environment, or arbundles createData + SolanaSigner) and POST it to Turbo. Under 100 KB is free. Never paste a private key into an agent.",
        "then":"san card fetch <txid>  (gateway: ".to_string() + &gateway() + ")"
    }))
}

pub fn graphql_query(wallet: Option<&str>, first: u32) -> Value {
    let mut tags = vec![json!({"name":"App-Name","values":[APP_NAME]}), json!({"name":"Type","values":["network-card"]})];
    if let Some(w) = wallet { tags.push(json!({"name":"Wallet","values":[w]})); }
    json!({"query":"query($tags:[TagFilter!],$first:Int){transactions(tags:$tags,first:$first,sort:HEIGHT_DESC){edges{node{id tags{name value}}}}}",
           "variables":{"tags":tags,"first":first}})
}

pub fn parse_graphql_ids(v: &Value) -> Vec<String> {
    v.pointer("/data/transactions/edges").and_then(Value::as_array).map(|e| e.iter()
        .filter_map(|x| x.pointer("/node/id").and_then(Value::as_str)).filter(|s| is_txid(s)).map(String::from).collect()).unwrap_or_default()
}

pub fn fetch_card(gw: &str, txid: &str) -> Result<(NetworkCard, String)> {
    if !is_txid(txid) { bail!("not an Arweave tx id: {txid}"); }
    let v: Value = ureq::get(&format!("{gw}/{txid}")).timeout(std::time::Duration::from_secs(20)).call()?.into_json()?;
    let mut c = card::parse(&v)?;
    c.endpoints.insert("arweave".into(), format!("{gw}/{txid}"));
    Ok((c, txid.into()))
}

/// Discover archived cards via gateway GraphQL (max 50), skipping invalid ones.
pub fn discover_cards(gw: &str, wallet: Option<&str>) -> Result<Vec<NetworkCard>> {
    let v: Value = ureq::post(&format!("{gw}/graphql")).timeout(std::time::Duration::from_secs(20))
        .send_json(graphql_query(wallet, 50))?.into_json()?;
    if let Some(e) = v.get("errors") { return Err(anyhow!("graphql: {e}")); }
    Ok(parse_graphql_ids(&v).iter().filter_map(|id| fetch_card(gw, id).ok().map(|x| x.0)).collect())
}
