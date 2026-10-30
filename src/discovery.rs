//! Peer discovery: merge registry agent records, onchain AgentNode accounts and published cards.
use crate::{card::NetworkCard, chain::NodeAccount};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub id: String,
    pub name: Option<String>,
    pub asset: Option<String>,
    pub wallet: Option<String>,
    pub onchain: bool,
    pub registry: bool,
    pub card: Option<NetworkCard>,
}

/// Registry response `{"ok":true,"agents":[...]}` -> nodes. Fields are untrusted data.
pub fn from_registry(v: &Value) -> Vec<Node> {
    let list = v.get("agents").and_then(Value::as_array).cloned().unwrap_or_default();
    list.iter().filter_map(|a| {
        let s = |k: &str| a.get(k).and_then(Value::as_str).map(|x| x.chars().take(128).collect::<String>());
        let asset = s("asset").or_else(|| s("address"));
        let id = asset.clone().or_else(|| s("id"))?;
        Some(Node { id, name: s("name"), asset, wallet: s("owner").or_else(|| s("authority")).or_else(|| s("wallet")), registry: true, ..Default::default() })
    }).collect()
}

pub fn merge(registry: Vec<Node>, onchain: &[NodeAccount], cards: &[NetworkCard]) -> Vec<Node> {
    let mut by: BTreeMap<String, Node> = registry.into_iter().map(|n| (n.id.clone(), n)).collect();
    for a in onchain {
        by.entry(a.pubkey.clone()).or_insert_with(|| Node { id: a.pubkey.clone(), ..Default::default() }).onchain = true;
    }
    for c in cards {
        let key = c.asset.clone().unwrap_or_else(|| c.agent_id.clone());
        let n = by.entry(key.clone()).or_insert_with(|| Node { id: key, ..Default::default() });
        n.name = n.name.clone().or(c.name.clone());
        n.wallet = n.wallet.clone().or(Some(c.wallet.clone()));
        n.asset = n.asset.clone().or(c.asset.clone());
        n.card = Some(c.clone());
    }
    by.into_values().collect()
}
