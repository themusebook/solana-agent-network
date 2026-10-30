//! Local state (~/.san or $SAN_HOME): this node's card, known cards, mailbox. Never stores keys.
use crate::{card::NetworkCard, mailbox::Mailbox};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Default, Serialize, Deserialize)]
pub struct State {
    pub me: Option<NetworkCard>,
    pub cards: BTreeMap<String, NetworkCard>,
    pub mailbox: Mailbox,
}

pub fn home() -> PathBuf {
    std::env::var_os("SAN_HOME").map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".san"))
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

impl State {
    pub fn path() -> PathBuf { home().join("state.json") }
    pub fn load() -> Result<Self> {
        match std::fs::read(Self::path()) {
            Ok(b) => Ok(serde_json::from_slice(&b)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }
    pub fn save(&self) -> Result<()> {
        let p = Self::path();
        std::fs::create_dir_all(p.parent().unwrap())?;
        let tmp = p.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(tmp, p)?;
        Ok(())
    }
    pub fn upsert_card(&mut self, c: NetworkCard) { self.cards.insert(c.agent_id.clone(), c); }
}
