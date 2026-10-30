//! x402m mailbox (ported from bridge/x402m-local.mjs). Sends are stored as PROPOSALS:
//! nothing here settles, signs, or transfers funds.
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Peer { pub agent_id: String, pub handle: String, pub name: String }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String, pub sender: String, pub recipient: String, pub request_id: String,
    pub kind: String, pub content: String, pub ack_at: Option<u64>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct SendResult { pub id: String, pub duplicate: bool, pub kind: String }

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Mailbox { pub peers: BTreeMap<String, Peer>, pub messages: Vec<Message> }

const MAX_CONTENT: usize = 16 * 1024;

impl Mailbox {
    pub fn register(&mut self, agent_id: &str, handle: &str, name: &str) -> Result<Peer> {
        if agent_id.trim().is_empty() || name.trim().is_empty() { bail!("register requires agentId, handle, and name"); }
        let handle = crate::card::normalize_handle(handle)?;
        if self.peers.values().any(|p| p.handle == handle && p.agent_id != agent_id) { bail!("handle taken"); }
        let p = Peer { agent_id: agent_id.into(), handle, name: name.trim().into() };
        self.peers.insert(agent_id.into(), p.clone());
        Ok(p)
    }
    fn resolve(&self, to: &str) -> Option<&Peer> {
        match to.strip_prefix('@') {
            Some(h) => { let h = h.to_ascii_lowercase(); self.peers.values().find(|p| p.handle == h) }
            None => self.peers.get(to),
        }
    }
    pub fn send(&mut self, from: &str, to: &str, request_id: &str, content: &str) -> Result<SendResult> {
        if !self.peers.contains_key(from) { bail!("register your messaging card first"); }
        let Some(rcpt) = self.resolve(to).map(|p| p.agent_id.clone()) else { bail!("recipient has not registered a messaging card") };
        if request_id.trim().is_empty() { bail!("requestId required"); }
        if content.trim().is_empty() { bail!("content required"); }
        if content.len() > MAX_CONTENT { bail!("content too large"); }
        if let Some(m) = self.messages.iter().find(|m| m.sender == from && m.request_id == request_id) {
            if m.content != content { bail!("requestId reused with different content"); }
            return Ok(SendResult { id: m.id.clone(), duplicate: true, kind: m.kind.clone() });
        }
        let id = uuid::Uuid::new_v4().to_string();
        self.messages.push(Message { id: id.clone(), sender: from.into(), recipient: rcpt, request_id: request_id.into(),
            kind: "proposal".into(), content: content.into(), ack_at: None });
        Ok(SendResult { id, duplicate: false, kind: "proposal".into() })
    }
    pub fn inbox(&self, agent_id: &str) -> Vec<Message> {
        self.messages.iter().filter(|m| m.recipient == agent_id && m.ack_at.is_none()).cloned().collect()
    }
    pub fn ack(&mut self, agent_id: &str, id: &str) -> Result<u64> {
        let Some(m) = self.messages.iter_mut().find(|m| m.id == id && m.recipient == agent_id) else { bail!("message not found in your inbox") };
        let now = crate::store::now_ms();
        Ok(*m.ack_at.get_or_insert(now))
    }
}
