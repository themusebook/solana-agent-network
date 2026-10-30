//! On-chain inbox (port of solana/inbox.mjs). Authority = fee payer; content is untrusted.
use super::envelope::{Envelope, MemoAssembler};
use super::tx::{MEMO_PROGRAM, SYSTEM_PROGRAM};
use anyhow::Result;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;

pub const MEMO_PROGRAM_LEGACY: &str = "Memo1UhkJRfHyvLMcVucJwxXeuD728EqVDDwQDxFMNo";

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedMail { pub sender_wallet: String, pub recipient_wallet: Option<String>, pub memos: Vec<String> }

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OnchainMessage {
    pub id: String, pub sender: String, pub sender_wallet: String, pub recipient: String, pub recipient_wallet: Option<String>,
    pub kind: String, pub content: String, pub conversation_id: Option<String>, pub reply_to: Option<String>,
    pub pay: Option<super::envelope::Pay>, pub ts: u64, pub signature: String, pub slot: u64, pub block_time: Option<i64>, pub transport: String,
}

fn accounts(tx: &Value) -> Vec<String> {
    let s = |v: &Value| v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect::<Vec<_>>()).unwrap_or_default();
    let mut a = s(&tx["transaction"]["message"]["accountKeys"]);
    a.extend(s(&tx["meta"]["loadedAddresses"]["writable"])); a.extend(s(&tx["meta"]["loadedAddresses"]["readonly"]));
    a
}

pub fn parse_mail_transaction(tx: &Value) -> Option<ParsedMail> {
    let acc = accounts(tx);
    let sender = acc.first()?.clone();
    let (mut rcpt, mut memos) = (None, vec![]);
    for ix in tx["transaction"]["message"]["instructions"].as_array()? {
        let prog = acc.get(ix["programIdIndex"].as_u64()? as usize)?;
        if prog == MEMO_PROGRAM || prog == MEMO_PROGRAM_LEGACY {
            if let Some(d) = ix["data"].as_str().and_then(|d| bs58::decode(d).into_vec().ok()).and_then(|b| String::from_utf8(b).ok()) { memos.push(d); }
        }
        if prog == SYSTEM_PROGRAM {
            if let Some(to) = ix["accounts"].as_array().filter(|a| a.len() >= 2).and_then(|a| a[1].as_u64()).and_then(|i| acc.get(i as usize)) {
                if *to != sender { rcpt = Some(to.clone()); }
            }
        }
    }
    (!memos.is_empty()).then_some(ParsedMail { sender_wallet: sender, recipient_wallet: rcpt, memos })
}

fn to_msg(tx: &Value, p: &ParsedMail, e: Envelope) -> OnchainMessage {
    OnchainMessage { id: e.id, sender: e.from, sender_wallet: p.sender_wallet.clone(), recipient: e.to, recipient_wallet: p.recipient_wallet.clone(),
        kind: e.kind, content: e.content, conversation_id: e.conversation_id, reply_to: e.reply_to, pay: e.pay, ts: e.ts,
        signature: tx["transaction"]["signatures"][0].as_str().unwrap_or("").into(), slot: tx["slot"].as_u64().unwrap_or(0),
        block_time: tx["blockTime"].as_i64(), transport: "solana-memo".into() }
}

/// Extract messages from getTransaction (encoding "json") results; `inbox_of` keeps only mail TO that wallet.
pub fn messages_from_transactions(txs: &[Value], inbox_of: Option<&str>) -> Vec<OnchainMessage> {
    let mut asm: HashMap<String, MemoAssembler> = HashMap::new();
    let mut out = vec![];
    for tx in txs {
        if !tx["meta"]["err"].is_null() { continue; }
        let Some(p) = parse_mail_transaction(tx) else { continue };
        if let Some(w) = inbox_of { if p.recipient_wallet.as_deref() != Some(w) { continue; } }
        let a = asm.entry(p.sender_wallet.clone()).or_default();
        for m in &p.memos { if let Some(e) = a.feed(&p.sender_wallet, m) { out.push(to_msg(tx, &p, e)); } }
    }
    out
}

fn rpc(url: &str, method: &str, params: Value) -> Result<Value> {
    crate::config::guard_devnet(url)?;
    let v: Value = ureq::post(url).send_json(json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))?.into_json()?;
    if let Some(e) = v.get("error") { anyhow::bail!("rpc {method}: {e}"); }
    Ok(v["result"].clone())
}

/// Scan a devnet wallet's recent signatures and return its on-chain x402m inbox.
pub fn fetch_onchain_inbox(rpc_url: &str, wallet: &str, limit: u32) -> Result<Vec<OnchainMessage>> {
    let sigs = rpc(rpc_url, "getSignaturesForAddress", json!([wallet, {"limit": limit.min(100)}]))?;
    let mut txs = vec![];
    for s in sigs.as_array().cloned().unwrap_or_default() {
        if !s["err"].is_null() { continue; }
        let Some(sig) = s["signature"].as_str() else { continue };
        let t = rpc(rpc_url, "getTransaction", json!([sig, {"encoding":"json","maxSupportedTransactionVersion":0}]))?;
        if !t.is_null() { txs.push(t); }
    }
    Ok(messages_from_transactions(&txs, Some(wallet)))
}

pub fn latest_blockhash(rpc_url: &str) -> Result<String> {
    let r = rpc(rpc_url, "getLatestBlockhash", json!([{"commitment":"finalized"}]))?;
    r["value"]["blockhash"].as_str().map(String::from).ok_or_else(|| anyhow::anyhow!("no blockhash"))
}
