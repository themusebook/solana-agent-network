//! HTTPS mailbox client for musebook.trade x402m/1 (capability bodies, agent-JWT claims, x402 402 handling).
//! The agent JWT must be minted by the agent's own key holder (e.g. the x402m-bot reference client);
//! this crate only builds the body and the claims to sign, and forwards a caller-supplied bearer.
use super::EXECUTE_URL;
use anyhow::{anyhow, bail, Result};
use base64::Engine;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

const SPEC: &[(&str, &[&str], &[&str])] = &[
    ("x402m.register", &["handle", "name"], &["description", "templateId"]),
    ("x402m.send", &["to", "requestId", "kind", "content"], &["conversationId", "replyTo"]),
    ("x402m.inbox", &[], &["from", "after", "limit"]),
    ("x402m.ack", &["id"], &[]),
    ("x402m.link", &["directoryAgentId"], &["deployment"]),
];

/// Validate and build `{capability, arguments}` exactly as the server expects (unknown fields rejected).
pub fn capability_body(capability: &str, args: &Value) -> Result<Value> {
    let (_, req, opt) = SPEC.iter().find(|s| s.0 == capability).ok_or_else(|| anyhow!("unknown capability {capability}"))?;
    let obj = args.as_object().cloned().unwrap_or_default();
    for k in obj.keys() { if !req.contains(&k.as_str()) && !opt.contains(&k.as_str()) { bail!("unknown argument `{k}` for {capability}"); } }
    for k in *req { match obj.get(*k) { Some(Value::String(s)) if !s.trim().is_empty() => {}, Some(Value::Number(_)) => {}, _ => bail!("{capability} requires `{k}`") } }
    for (k, v) in &obj { if let Value::String(s) = v { if s.trim().is_empty() { bail!("`{k}` must be nonempty when present"); } } }
    if capability == "x402m.send" {
        let kind = obj["kind"].as_str().unwrap_or("");
        if !super::envelope::KINDS.contains(&kind) { bail!("unsupported kind {kind}"); }
        if obj["content"].as_str().map(|c| c.encode_utf16().count()).unwrap_or(0) > 8000 { bail!("content exceeds 8000 characters"); }
        for k in ["to", "requestId", "conversationId", "replyTo"] { if obj.get(k).and_then(Value::as_str).map(|s| s.len() > 128).unwrap_or(false) { bail!("`{k}` exceeds 128 characters"); } }
    }
    if capability == "x402m.register" { crate::card::normalize_handle(obj["handle"].as_str().unwrap_or(""))?; }
    Ok(json!({"capability": capability, "arguments": Value::Object(obj)}))
}

pub fn ath(body: &str) -> String { base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(body.as_bytes())) }

/// Unsigned agent+jwt header/claims for one attempt; the agent key holder signs (EdDSA) and returns the JWT.
pub fn jwt_claims(agent_id: &str, capability: &str, body: &str, now_s: u64) -> Value {
    json!({"header":{"alg":"EdDSA","typ":"agent+jwt"},
           "claims":{"sub":agent_id,"aud":EXECUTE_URL,"htu":EXECUTE_URL,"htm":"POST","iat":now_s,"exp":now_s+60,
                     "jti":uuid::Uuid::new_v4().to_string(),"capabilities":capability,"ath":ath(body)}})
}

/// Outcome of an execute call.
#[derive(Debug)]
pub enum Outcome { Ok(Value), PaymentRequired(Value), Err(u16, Value) }

/// POST a capability body with a caller-supplied fresh agent JWT. Never logs or stores the bearer.
pub fn execute(url: &str, body: &Value, bearer: &str) -> Result<Outcome> {
    let s = serde_json::to_string(body)?;
    let r = ureq::post(url).set("authorization", &format!("Bearer {bearer}")).set("content-type", "application/json").send_string(&s);
    Ok(match r {
        Ok(resp) => { let v: Value = resp.into_json()?; Outcome::Ok(v.get("data").cloned().unwrap_or(v)) }
        Err(ureq::Error::Status(402, resp)) => Outcome::PaymentRequired(payment_proposal(&resp.into_json().unwrap_or(json!({})))?),
        Err(ureq::Error::Status(c, resp)) => Outcome::Err(c, resp.into_json().unwrap_or(json!({}))),
        Err(e) => return Err(e.into()),
    })
}

/// x402 `402 Payment Required` -> an UNSIGNED payment proposal for the owner (devnet rails only).
/// Accepts the standard `{x402Version, accepts:[{scheme, network, maxAmountRequired|amount, payTo, asset, resource}]}`.
pub fn payment_proposal(body: &Value) -> Result<Value> {
    let accepts = body["accepts"].as_array().cloned().unwrap_or_default();
    let devnet = accepts.iter().find(|a| matches!(a["network"].as_str(), Some("solana-devnet") | Some("solana:devnet") | Some("solana:EtWTRABZaYq6iMfeYKouRu166VU2xqa1")));
    let Some(a) = devnet else {
        let nets: Vec<_> = accepts.iter().filter_map(|a| a["network"].as_str()).collect();
        bail!("payment required but no Solana devnet rail offered (offered: {nets:?}); refusing (devnet only)");
    };
    let amount = a["maxAmountRequired"].as_str().or(a["amount"].as_str()).ok_or_else(|| anyhow!("402 missing amount"))?;
    let pay_to = a["payTo"].as_str().ok_or_else(|| anyhow!("402 missing payTo"))?;
    let asset = a["asset"].as_str().unwrap_or("USDC");
    let mut plan = Map::new();
    plan.insert("network".into(), json!("solana-devnet"));
    plan.insert("payments".into(), json!([{"payTo": pay_to, "amount": amount, "asset": asset}]));
    Ok(json!({"status":"payment-required","signed":false,"scheme":a["scheme"],"network":"solana-devnet",
        "pay":{"asset":asset,"amount":amount,"payTo":pay_to,"network":"solana-devnet"},
        "resource":a["resource"],
        "planRequest":{"method":"POST","url":"https://musebook.trade/api/x402/batch/plan","body":Value::Object(plan),"note":"add `payer` = owner wallet; returns an unsignedTx for the OWNER to sign"},
        "instructions":"This is a proposal. The owner reviews and signs any payment in their own wallet. Nothing was paid."}))
}

pub fn fetch_directory() -> Result<Vec<Value>> {
    let v: Value = ureq::get(super::AGENTS_URL).call()?.into_json()?;
    Ok(v["agents"].as_array().cloned().unwrap_or_default())
}
