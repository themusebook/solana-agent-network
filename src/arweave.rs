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
/// Public read fallback. io.musebook.trade (AR.IO release 84) has ANS-104 unbundling/indexing disabled, so its
/// GraphQL returns no bundled data items, and `/{txid}` 302s to a sandbox subdomain of ao.musebook.trade whose
/// TLS cert does not cover it. Reads fall back here; nothing is ever written here.
pub const FALLBACK_GATEWAY: &str = "https://arweave.net";

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
    let mut last = anyhow!("no gateway");
    for base in gateways(gw) {
        // /raw/ skips the sandbox-subdomain redirect.
        let url = format!("{base}/raw/{txid}");
        match ureq::get(&url).timeout(std::time::Duration::from_secs(20)).call() {
            Ok(r) => {
                let v: Value = r.into_json()?;
                let mut c = card::parse(&v)?;
                c.endpoints.insert("arweave".into(), format!("{base}/{txid}"));
                return Ok((c, txid.into()));
            }
            Err(e) => last = anyhow!("{url}: {e}"),
        }
    }
    Err(last)
}

/// Tries the configured gateway first, then the public fallback if it returns nothing.
pub fn discover_cards(gw: &str, wallet: Option<&str>) -> Result<Vec<NetworkCard>> {
    let mut last = None;
    for base in gateways(gw) {
        let r = ureq::post(&format!("{base}/graphql")).timeout(std::time::Duration::from_secs(20)).send_json(graphql_query(wallet, 50));
        let v: Value = match r { Ok(r) => r.into_json()?, Err(e) => { last = Some(anyhow!("graphql {base}: {e}")); continue; } };
        if let Some(e) = v.get("errors") { last = Some(anyhow!("graphql {base}: {e}")); continue; }
        let ids = parse_graphql_ids(&v);
        if ids.is_empty() { continue; }
        return Ok(ids.iter().filter_map(|id| fetch_card(gw, id).ok().map(|x| x.0)).collect());
    }
    match last { Some(e) => Err(e), None => Ok(vec![]) }
}

/// Configured gateway first, then the public fallback (deduplicated).
pub fn gateways(gw: &str) -> Vec<String> {
    let gw = gw.trim_end_matches('/').to_string();
    if gw == FALLBACK_GATEWAY { vec![gw] } else { vec![gw, FALLBACK_GATEWAY.into()] }
}

/// Unsigned heartbeat data item: a tiny, permanent liveness record for an on-chain AgentNode.
/// Same signing model as cards (owner wallet signs, Turbo free tier < 100 KB); never signed or paid here.
pub fn heartbeat_payload(agent_id: &str, wallet: &str, node_pda: Option<&str>, ts_ms: u64) -> Result<Value> {
    if agent_id.is_empty() || !crate::config::is_pubkey(wallet) { bail!("agentId and a base58 wallet are required"); }
    let mut body = json!({"schema":"san-heartbeat/v1","agentId":agent_id,"wallet":wallet,"network":crate::config::NETWORK,
        "programId":crate::config::PROGRAM_ID,"ts":ts_ms});
    let mut tags: Vec<(&str, String)> = vec![("Content-Type", "application/json".into()), ("App-Name", APP_NAME.into()),
        ("App-Version", env!("CARGO_PKG_VERSION").into()), ("Type", "heartbeat".into()), ("Network", crate::config::NETWORK.into()),
        ("Wallet", wallet.into()), ("Agent-Id", agent_id.into())];
    if let Some(p) = node_pda {
        if !crate::config::is_pubkey(p) { bail!("nodePda must be base58"); }
        body["nodePda"] = json!(p);
        tags.push(("Node-Pda", p.into()));
    }
    let data = body.to_string();
    let tags: Vec<Value> = tags.into_iter().map(|(n, v)| json!({"name":n,"value":v})).collect();
    Ok(json!({"format":"ANS-104 data item (unsigned)","signatureType":2,"signer":"owner Solana wallet (ed25519)",
        "upload":{"method":"POST","url":TURBO_UPLOAD,"contentType":"application/octet-stream"},"bytes":data.len(),"freeTier":true,
        "tags":tags,"data":data,
        "instructions":"Owner signs and POSTs to Turbo (free under 100 KB). No AR needed; this node never signs or pays."}))
}

fn probe(u: &str) -> Value {
    match ureq::get(u).timeout(std::time::Duration::from_secs(10)).call() {
        Ok(r) => { let code = r.status(); let v: Value = r.into_json().unwrap_or(Value::Null); json!({"ok":true,"status":code,"body":v}) }
        Err(ureq::Error::Status(c, _)) => json!({"ok":false,"status":c}),
        Err(e) => json!({"ok":false,"error":e.to_string()}),
    }
}

/// Read-only health of the configured AR.IO gateway and AO gateway. Never pays the x402 compute quote.
pub fn status(gw: &str, ao: &str) -> Value {
    let info = probe(&format!("{gw}/ar-io/info"));
    let s = probe(&format!("{ao}/gateway/api/status"));
    json!({"ok":true,
        "arweave":{"gateway":gw,"healthcheck":probe(&format!("{gw}/ar-io/healthcheck")),
            "release":info.pointer("/body/release"),
            "ans104Indexing":!info.pointer("/body/ans104IndexFilter/never").and_then(Value::as_bool).unwrap_or(false),
            "fallback":FALLBACK_GATEWAY},
        "ao":{"url":ao,"hyperbeamReachable":s.pointer("/body/services/hyperbeam/reachable"),
            "arioReachable":s.pointer("/body/services/ario/reachable"),
            "computePayments":s.pointer("/body/payments"),
            "note":"AO compute at /gateway/ao/:process/compute?slot=N is x402-paid (testnet USDC on Base Sepolia); this node only reads status and never pays."}})
}
