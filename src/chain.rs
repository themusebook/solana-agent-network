//! Direct devnet reads of SAN program accounts (AgentNode) via JSON-RPC.
use crate::config::{guard_devnet, PROGRAM_ID};
use anyhow::{anyhow, Result};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct NodeAccount { pub pubkey: String, pub lamports: u64, pub data_len: usize }

pub fn program_accounts_request() -> Value {
    json!({"jsonrpc":"2.0","id":1,"method":"getProgramAccounts",
        "params":[PROGRAM_ID, {"encoding":"base64","commitment":"confirmed"}]})
}

pub fn parse_program_accounts(v: &Value) -> Result<Vec<NodeAccount>> {
    if let Some(e) = v.get("error") { return Err(anyhow!("rpc error: {e}")); }
    let arr = v.get("result").and_then(Value::as_array).ok_or_else(|| anyhow!("rpc: missing result"))?;
    Ok(arr.iter().filter_map(|a| {
        let data_len = a.pointer("/account/data/0").and_then(Value::as_str).map(|s| s.len() * 3 / 4).unwrap_or(0);
        Some(NodeAccount {
            pubkey: a.get("pubkey")?.as_str()?.to_string(),
            lamports: a.pointer("/account/lamports").and_then(Value::as_u64).unwrap_or(0),
            data_len,
        })
    }).collect())
}

pub fn fetch_nodes(rpc: &str) -> Result<Vec<NodeAccount>> {
    guard_devnet(rpc)?;
    let v: Value = ureq::post(rpc).send_json(program_accounts_request())?.into_json()?;
    parse_program_accounts(&v)
}
