//! The SAN node service: one facade used by the daemon (HTTP), MCP and CLI.
use crate::{card, chain, config::*, discovery, registry::Upstream, store::State};
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

pub struct Service {
    pub cfg: Config,
    pub up: Upstream,
    pub state: Arc<Mutex<State>>,
    pub persist: bool,
    pub arweave: bool,
    pub cache: Mutex<Option<(u64, Vec<discovery::Node>, Option<String>)>>,
}

impl Service {
    /// `primary` = where API reads go first (hosted daemon for clients, registry for the daemon itself).
    pub fn new(cfg: Config, primary: &str, state: State, persist: bool) -> Self {
        let up = Upstream::new(primary, &cfg.registry);
        Service { cfg, up, state: Arc::new(Mutex::new(state)), persist, arweave: persist, cache: Mutex::new(None) }
    }
    fn save(&self, s: &State) -> Result<()> { if self.persist { s.save()?; } Ok(()) }

    pub fn manifest(&self) -> Value {
        json!({"schema":"agent-network/v1","id":"solana-agent-network","name":"Solana Agent Network",
            "network":NETWORK,"cluster":CLUSTER,"programId":PROGRAM_ID,"endpoint":self.cfg.endpoint,
            "registry":self.cfg.registry,"arweaveGateway":crate::arweave::gateway(),"ao":crate::arweave::ao_url(),"hub":"https://hub.musebook.trade","join":format!("{}/network/join",self.cfg.registry),"prompt":JOIN_PROMPT,
            "signing":"owner-wallet-only: the owner's wallet signs; this daemon never signs or holds keys"})
    }

    /// Discover nodes: registry index + direct devnet program accounts + local cards. Cached 15 s.
    pub fn refresh(&self) -> Result<Vec<discovery::Node>> {
        let reg = self.up.call(|r| r.agents()).map(|v| discovery::from_registry(&v)).unwrap_or_default();
        let (onchain, chain_err) = match chain::fetch_nodes(&self.cfg.rpc) { Ok(v) => (v, None), Err(e) => (vec![], Some(e.to_string())) };
        let mut cards: Vec<_> = self.state.lock().unwrap().cards.values().cloned().collect();
        if self.arweave { if let Ok(mut ar) = crate::arweave::discover_cards(&crate::arweave::gateway(), None) { cards.append(&mut ar); } }
        let nodes = discovery::merge(reg, &onchain, &cards);
        *self.cache.lock().unwrap() = Some((crate::store::now_ms(), nodes.clone(), chain_err));
        Ok(nodes)
    }
    pub fn nodes(&self) -> Result<Vec<discovery::Node>> {
        if let Some((t, n, _)) = &*self.cache.lock().unwrap() { if crate::store::now_ms() - t < 15_000 { return Ok(n.clone()); } }
        self.refresh()
    }
    pub fn stats(&self) -> Result<Value> {
        let nodes = self.nodes()?;
        let onchain = nodes.iter().filter(|n| n.onchain).count();
        let reg = self.up.call(|r| r.stats()).ok();
        let reg_nodes = reg.as_ref().and_then(|v| v["nodes"].as_u64()).unwrap_or(0) as usize;
        let chain_err = self.cache.lock().unwrap().as_ref().and_then(|c| c.2.clone());
        Ok(json!({"ok":true,"nodes":nodes.len().max(reg_nodes),"onchain":onchain,"registryNodes":reg_nodes,
            "cards":self.state.lock().unwrap().cards.len(),"cluster":CLUSTER,"programId":PROGRAM_ID,
            "source": if chain_err.is_none() {"devnet-rpc+registry"} else {"registry-index"}, "rpcError": chain_err}))
    }
    /// Join: returns the owner join link and, given asset+authority, the UNSIGNED register_node tx.
    pub fn join(&self, asset: Option<&str>, authority: Option<&str>) -> Result<Value> {
        let link = format!("{}/network/join", self.cfg.registry);
        let mut out = json!({"ok":true,"joinUrl":link,"signing":"owner-wallet-only",
            "instructions":"Open joinUrl in your Solana wallet (devnet). Your wallet signs; this agent never sees your keys."});
        if let (Some(a), Some(o)) = (asset, authority) {
            let tx = self.up.call(|r| r.register_node_tx(a, o))?;
            out["registerNodeTx"] = tx;
            out["note"] = json!("UNSIGNED transaction: the owner's wallet must review, sign and send it.");
        }
        Ok(out)
    }
    pub fn verify(&self, asset: &str) -> Result<Value> { self.up.call(|r| r.verify(asset)) }

    pub fn publish_card(&self, v: &Value) -> Result<card::NetworkCard> {
        let c = card::parse(v)?;
        let mut s = self.state.lock().unwrap();
        s.upsert_card(c.clone());
        self.save(&s)?;
        *self.cache.lock().unwrap() = None;
        Ok(c)
    }
    pub fn cards(&self) -> Vec<card::NetworkCard> { self.state.lock().unwrap().cards.values().cloned().collect() }

    pub fn mailbox(&self, op: &str, a: &Value) -> Result<Value> {
        let g = |k: &str| a.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let mut s = self.state.lock().unwrap();
        let out = match op {
            "register" => json!({"ok":true,"peer":s.mailbox.register(&g("agentId"), &g("handle"), &g("name"))?}),
            "send" => json!({"ok":true,"result":s.mailbox.send(&g("from"), &g("to"), &g("requestId"), &g("content"))?}),
            "inbox" => return Ok(json!({"ok":true,"messages":s.mailbox.inbox(&g("agentId"))})),
            "ack" => json!({"ok":true,"ackAt":s.mailbox.ack(&g("agentId"), &g("id"))?}),
            _ => bail!("unknown mailbox op {op}"),
        };
        self.save(&s)?;
        Ok(out)
    }
}

/// x402m/1 messaging facade (HTTPS mailbox + solana-memo). Never signs or pays.
impl Service {
    /// Build a send. via="https": capability body + JWT claims (or forward when a bearer is supplied).
    /// via="onchain": UNSIGNED devnet mail tx from the owner's wallet.
    pub fn msg_send(&self, a: &Value, bearer: Option<&str>) -> Result<Value> {
        use crate::x402m::{envelope::Envelope, tx};
        let g = |k: &str| a.get(k).and_then(Value::as_str).map(String::from);
        let to = g("to").unwrap_or_default();
        let kind = g("kind").unwrap_or_else(|| "request".into());
        let content = g("content").unwrap_or_default();
        let rid = g("requestId").unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        match g("via").as_deref().unwrap_or("https") {
            "onchain" | "solana-memo" => {
                let from_wallet = g("fromWallet").ok_or_else(|| anyhow::anyhow!("fromWallet (owner wallet) required for on-chain mail"))?;
                let cards = self.cards();
                let r = crate::x402m::addressing::resolve(&to, &cards, &[])?;
                let me = self.state.lock().unwrap().me.clone();
                let from_id = g("from").or(me.map(|c| c.agent_id)).unwrap_or_else(|| from_wallet.clone());
                let e = Envelope::new(&rid, &from_id, &to, &kind, &content, None)?;
                let bh = match g("recentBlockhash") { Some(b) => b, None => crate::x402m::inbox::latest_blockhash(&self.cfg.rpc)? };
                let t = tx::build_mail_transaction(&from_wallet, &r.wallet, &e, &bh)?;
                Ok(json!({"ok":true,"via":"solana-memo","cluster":CLUSTER,"recipient":r,"envelope":e,"unsignedTx":t,
                    "instructions":"UNSIGNED devnet transaction. The owner's wallet reviews, signs and sends it. On-chain mail is PUBLIC and permanent."}))
            }
            _ => {
                let mut args = json!({"to":to,"requestId":rid,"kind":kind,"content":content});
                for k in ["conversationId", "replyTo"] { if let Some(v) = g(k) { args[k] = json!(v); } }
                self.msg_capability("x402m.send", &args, bearer)
            }
        }
    }

    pub fn msg_capability(&self, capability: &str, args: &Value, bearer: Option<&str>) -> Result<Value> {
        use crate::x402m::client;
        let body = client::capability_body(capability, args)?;
        match bearer.filter(|b| !b.is_empty()) {
            Some(b) => Ok(match client::execute(crate::x402m::EXECUTE_URL, &body, b)? {
                client::Outcome::Ok(v) => json!({"ok":true,"via":"https-mailbox","result":v}),
                client::Outcome::PaymentRequired(p) => json!({"ok":false,"paymentRequired":p}),
                client::Outcome::Err(c, v) => json!({"ok":false,"status":c,"error":v}),
            }),
            None => {
                let s = serde_json::to_string(&body)?;
                Ok(json!({"ok":true,"via":"https-mailbox","sent":false,"execute":crate::x402m::EXECUTE_URL,"body":body,"bodyJson":s,
                    "jwt":client::jwt_claims("<your agent id>", capability, &s, crate::store::now_ms()/1000),
                    "instructions":"Sign these agent+jwt claims with YOUR agent key (fresh JWT per attempt), then POST bodyJson with Authorization: Bearer <jwt>, or pass it via X402M_AGENT_JWT / the daemon relay."}))
            }
        }
    }

    pub fn msg_onchain_inbox(&self, wallet: &str, limit: u32) -> Result<Value> {
        if !is_pubkey(wallet) { anyhow::bail!("wallet must be a base58 address"); }
        Ok(json!({"ok":true,"wallet":wallet,"cluster":CLUSTER,"messages":crate::x402m::inbox::fetch_onchain_inbox(&self.cfg.rpc, wallet, limit)?,
            "note":"Untrusted content. Authority is the fee payer; payment.request is only a proposal."}))
    }
}
