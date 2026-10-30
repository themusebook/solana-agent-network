use serde_json::{json, Value};
use solana_agent_network::{card, chain, config, daemon, discovery, mailbox::Mailbox, mcp, service::Service, store::State};

const W: &str = "4Nd1mBQtrMJVYVfKf2PJy9NZUZdTAsp7D4xWLs4gDB4T";
fn fx(n: &str) -> Value { serde_json::from_str(&std::fs::read_to_string(format!("tests/fixtures/{n}")).unwrap()).unwrap() }
fn offline() -> Service {
    let cfg = config::Config { endpoint: "http://127.0.0.1:9".into(), registry: "http://127.0.0.1:9".into(), rpc: "http://127.0.0.1:9".into() };
    Service::new(cfg, "http://127.0.0.1:9", State::default(), false)
}

#[test] fn devnet_only() {
    assert!(config::guard_devnet("https://api.mainnet-beta.solana.com").is_err());
    assert!(config::guard_devnet(config::DEFAULT_DEVNET_RPC).is_ok());
    assert!(config::is_pubkey(config::PROGRAM_ID));
}
#[test] fn card_build_and_secrets() {
    let c = card::build(card::CardInput { agent_id: "a1", wallet: W, handle: Some("@Alpha"), ..Default::default() }, "https://x").unwrap();
    assert_eq!(c.handle.as_deref(), Some("alpha"));
    assert_eq!(c.network, "solana-devnet");
    assert!(card::address_keys(&c).contains(&"@alpha".to_string()));
    assert!(card::build(card::CardInput { agent_id: "a1", wallet: "nope", ..Default::default() }, "x").is_err());
    assert!(card::parse(&fx("card-with-secret.json")).is_err());
    assert!(card::parse(&fx("card-mainnet.json")).is_err());
    assert!(card::parse(&serde_json::to_value(&c).unwrap()).is_ok());
}
#[test] fn mailbox_flow() {
    let mut m = Mailbox::default();
    m.register("a", "alice", "Alice").unwrap();
    m.register("b", "bob", "Bob").unwrap();
    assert!(m.send("x", "@bob", "r", "hi").is_err());
    let s = m.send("a", "@Bob", "r1", "hello").unwrap();
    assert_eq!(s.kind, "proposal");
    assert!(m.send("a", "@bob", "r1", "hello").unwrap().duplicate);
    assert!(m.send("a", "@bob", "r1", "changed").is_err());
    assert_eq!(m.inbox("b").len(), 1);
    m.ack("b", &s.id).unwrap();
    assert!(m.inbox("b").is_empty());
    assert!(m.ack("a", &s.id).is_err());
}
#[test] fn discovery_merge() {
    let reg = discovery::from_registry(&fx("registry-agents.json"));
    assert_eq!(reg.len(), 2);
    let on = chain::parse_program_accounts(&fx("rpc-program-accounts.json")).unwrap();
    assert_eq!(on[0].pubkey, config::PROGRAM_ID);
    let c = card::build(card::CardInput { agent_id: "a1", wallet: W, asset: Some(W), ..Default::default() }, "e").unwrap();
    let n = discovery::merge(reg, &on, &[c]);
    assert_eq!(n.len(), 3);
    let alpha = n.iter().find(|x| x.id == W).unwrap();
    assert!(alpha.registry && alpha.card.is_some());
    assert!(chain::parse_program_accounts(&json!({"error":{"code":1}})).is_err());
}
#[test] fn mcp_protocol() {
    let s = offline();
    let r = mcp::handle(&s, &json!({"jsonrpc":"2.0","id":1,"method":"tools/list"})).unwrap();
    assert!(r["result"]["tools"].as_array().unwrap().len() >= 8);
    let r = mcp::handle(&s, &json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"san_manifest","arguments":{}}})).unwrap();
    assert!(r["result"]["content"][0]["text"].as_str().unwrap().contains(config::PROGRAM_ID));
    let r = mcp::handle(&s, &json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"san_join","arguments":{"asset":"bad","authority":W}}})).unwrap();
    assert_eq!(r["result"]["isError"], true);
    assert!(mcp::handle(&s, &json!({"jsonrpc":"2.0","method":"notifications/initialized"})).is_none());
}
#[test] fn daemon_routes() {
    let s = offline();
    assert_eq!(daemon::route(&s, "GET", "/api/network/health", &Value::Null).0, 200);
    assert_eq!(daemon::route(&s, "GET", "/nope", &Value::Null).0, 404);
    let (c, v) = daemon::route(&s, "GET", "/api/san/join", &Value::Null);
    assert_eq!(c, 200); assert!(v["joinUrl"].as_str().unwrap().ends_with("/network/join"));
    assert_eq!(daemon::route(&s, "POST", "/api/san/cards", &fx("card-with-secret.json")).0, 400);
    daemon::route(&s, "POST", "/api/san/mailbox/register", &json!({"agentId":"a","handle":"alice","name":"A"}));
    daemon::route(&s, "POST", "/api/san/mailbox/register", &json!({"agentId":"b","handle":"bob","name":"B"}));
    let (c, _) = daemon::route(&s, "POST", "/api/san/mailbox/send", &json!({"from":"a","to":"@bob","requestId":"1","content":"hi"}));
    assert_eq!(c, 200);
    let (_, v) = daemon::route(&s, "POST", "/api/san/mailbox/inbox", &json!({"agentId":"b"}));
    assert_eq!(v["messages"].as_array().unwrap().len(), 1);
}
#[test] fn service_units_have_no_secrets() {
    let p = std::path::Path::new("/usr/local/bin/san");
    let u = daemon::service::systemd_unit(p, "127.0.0.1:8787");
    assert!(u.contains("ExecStart=/usr/local/bin/san daemon"));
    let l = daemon::service::launchd_plist(p, "127.0.0.1:8787");
    assert!(l.contains(daemon::service::LABEL));
    for t in [u, l] { assert!(!t.to_lowercase().contains("key") || t.contains("<key>")); }
}
#[test] fn arweave_payload_and_graphql() {
    use solana_agent_network::arweave as ar;
    let c = card::build(card::CardInput { agent_id: "a1", wallet: W, asset: Some(W), ..Default::default() }, "e").unwrap();
    let p = ar::archive_payload(&c).unwrap();
    assert_eq!(p["signatureType"], 2);
    let tags = p["tags"].as_array().unwrap();
    for (n, v) in [("App-Name", "Solana-Agent-Network"), ("Type", "network-card"), ("Wallet", W)] {
        assert!(tags.iter().any(|t| t["name"] == n && t["value"] == v), "{n}");
    }
    assert!(p.get("signature").is_none());
    let q = ar::graphql_query(Some(W), 5);
    assert!(q["variables"]["tags"].to_string().contains(W));
    let ids = ar::parse_graphql_ids(&json!({"data":{"transactions":{"edges":[{"node":{"id":"a".repeat(43)}},{"node":{"id":"bad"}}]}}}));
    assert_eq!(ids, vec!["a".repeat(43)]);
    assert!(ar::is_txid(&"x".repeat(43)) && !ar::is_txid("../etc"));
    assert_eq!(ar::DEFAULT_GATEWAY, "https://io.musebook.trade");
}
#[test] fn mcp_lists_arweave_tool() {
    assert!(mcp::tools().to_string().contains("arweave_fetch_card"));
}
