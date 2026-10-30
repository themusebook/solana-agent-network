//! x402m/1 envelope and SPL Memo encoding (port of solana/envelope.mjs).
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};

pub const MEMO_PREFIX: &str = "x402m/1 ";
pub const MEMO_PART_PREFIX: &str = "x402m/1>";
pub const MEMO_MAX_BYTES: usize = 566;
pub const MAX_PARTS: usize = 40;
pub const KINDS: &[&str] = &["request", "response", "event", "payment.request", "payment.receipt", "error"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Pay {
    pub asset: String, pub amount: String, pub pay_to: String,
    #[serde(skip_serializing_if = "Option::is_none")] pub network: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub r#ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub tx: Option<String>,
}

/// Field order matches JSON.stringify in the reference (v,id,from,to,kind,content,ts,...).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    pub v: u8, pub id: String, pub from: String, pub to: String, pub kind: String, pub content: String, pub ts: u64,
    #[serde(skip_serializing_if = "Option::is_none")] pub conversation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub reply_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub pay: Option<Pay>,
}

fn req(v: &str, f: &str, max: usize) -> Result<()> {
    // JS length = UTF-16 code units
    if v.trim().is_empty() || v.encode_utf16().count() > max { bail!("invalid envelope field: {f}"); }
    Ok(())
}

impl Envelope {
    pub fn new(id: &str, from: &str, to: &str, kind: &str, content: &str, ts: Option<u64>) -> Result<Self> {
        let e = Envelope { v: 1, id: id.into(), from: from.into(), to: to.into(), kind: kind.into(), content: content.into(),
            ts: ts.unwrap_or_else(crate::store::now_ms), conversation_id: None, reply_to: None, pay: None };
        e.validate()?;
        Ok(e)
    }
    pub fn validate(&self) -> Result<()> {
        if self.v != 1 { bail!("unsupported envelope version"); }
        req(&self.id, "id", 128)?; req(&self.from, "from", 128)?; req(&self.to, "to", 128)?;
        if !KINDS.contains(&self.kind.as_str()) { bail!("invalid envelope field: kind ({})", self.kind); }
        req(&self.content, "content", 8000)?;
        if let Some(c) = &self.conversation_id { req(c, "conversationId", 128)?; }
        if let Some(c) = &self.reply_to { req(c, "replyTo", 128)?; }
        if let Some(p) = &self.pay {
            req(&p.asset, "pay.asset", 128)?; req(&p.amount, "pay.amount", 128)?; req(&p.pay_to, "pay.payTo", 128)?;
        }
        Ok(())
    }
    pub fn serialize(&self) -> String { serde_json::to_string(self).expect("envelope serializes") }
}

/// Encode as memo strings for one transaction.
pub fn encode_memos(e: &Envelope) -> Result<Vec<String>> {
    let payload = e.serialize();
    let single = format!("{MEMO_PREFIX}{payload}");
    if single.len() <= MEMO_MAX_BYTES { return Ok(vec![single]); }
    let mut parts: Vec<String> = vec![];
    let chars: Vec<char> = payload.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let prefix = format!("{MEMO_PART_PREFIX}{} {}/{MAX_PARTS} ", e.id, parts.len() + 1);
        let budget = MEMO_MAX_BYTES.checked_sub(prefix.len()).filter(|b| *b > 0).ok_or_else(|| anyhow::anyhow!("envelope id too long to chunk"))?;
        let (mut bytes, start) = (0, i);
        while i < chars.len() && bytes + chars[i].len_utf8() <= budget { bytes += chars[i].len_utf8(); i += 1; }
        if i == start { bail!("single character exceeds memo budget"); }
        parts.push(chars[start..i].iter().collect());
        if parts.len() > MAX_PARTS { bail!("envelope too large: exceeds {MAX_PARTS} memo parts"); }
    }
    let n = parts.len();
    Ok(parts.into_iter().enumerate().map(|(k, c)| format!("{MEMO_PART_PREFIX}{} {}/{n} {c}", e.id, k + 1)).collect())
}

pub fn message_key(sender_wallet: &str, id: &str) -> String {
    let h = Sha256::digest(format!("{sender_wallet}:{id}"));
    h.iter().map(|b| format!("{b:02x}")).collect::<String>()[..32].to_string()
}

fn parse_payload(t: &str) -> Option<Envelope> {
    let e: Envelope = serde_json::from_str(t).ok()?;
    (e.v == 1 && KINDS.contains(&e.kind.as_str())).then_some(e)
}

/// Feed memos per sender wallet in any order; yields each envelope exactly once.
#[derive(Default)]
pub struct MemoAssembler { pending: HashMap<String, (usize, BTreeMap<usize, String>)>, seen: HashSet<String> }

impl MemoAssembler {
    pub fn feed(&mut self, sender: &str, memo: &str) -> Option<Envelope> {
        if let Some(p) = memo.strip_prefix(MEMO_PREFIX) { return parse_payload(p).and_then(|e| self.complete(sender, e)); }
        let rest = memo.strip_prefix(MEMO_PART_PREFIX)?;
        let (id, rest) = rest.split_once(' ')?;
        let (frac, chunk) = rest.split_once(' ').unwrap_or((rest, ""));
        let (a, b) = frac.split_once('/')?;
        if id.is_empty() || id.len() > 128 || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') || a.len() > 2 || b.len() > 2 { return None; }
        let (idx, total): (usize, usize) = (a.parse().ok()?, b.parse().ok()?);
        if idx < 1 || total < 1 || idx > total || total > MAX_PARTS { return None; }
        let key = message_key(sender, id);
        if self.seen.contains(&key) { return None; }
        let entry = self.pending.entry(key.clone()).or_insert((total, BTreeMap::new()));
        if entry.0 != total { return None; }
        entry.1.insert(idx, chunk.to_string());
        if entry.1.len() < total { return None; }
        let payload: String = self.pending.remove(&key)?.1.into_values().collect();
        let e = parse_payload(&payload)?;
        if e.id != id { return None; }
        self.complete(sender, e)
    }
    fn complete(&mut self, sender: &str, e: Envelope) -> Option<Envelope> {
        self.seen.insert(message_key(sender, &e.id)).then_some(e)
    }
}
