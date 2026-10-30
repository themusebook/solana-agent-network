//! Interop with github.com/solizardking/x402-messaging using its own fixtures (tests/fixtures/x402m).
use serde_json::{json, Value};
use solana_agent_network::{card, x402m::{addressing, client, envelope::{self, Envelope, MemoAssembler}, inbox, tx}};

fn mail() -> Value { serde_json::from_str(&std::fs::read_to_string("tests/fixtures/x402m/mail.json").unwrap()).unwrap() }

#[test] fn mail_tx_is_byte_identical_to_reference() {
    let f = mail();
    let e: Envelope = serde_json::from_value(f["envelope"].clone()).unwrap();
    let t = tx::build_mail_transaction(f["from"].as_str().unwrap(), f["to"].as_str().unwrap(), &e, f["blockhash"].as_str().unwrap()).unwrap();
    assert_eq!(t.transaction_base64, f["singleTxBase64"].as_str().unwrap());
    assert_eq!(t.message_id, "req-0001");
    assert_eq!(t.lamports, 0);
    assert!(!t.signed);
    assert!(tx::build_mail_transaction("nope", f["to"].as_str().unwrap(), &e, f["blockhash"].as_str().unwrap()).unwrap_err().to_string().contains("base58"));
    assert!(tx::build_mail_transaction(f["from"].as_str().unwrap(), f["to"].as_str().unwrap(), &e, "nope").unwrap_err().to_string().contains("blockhash"));
}

#[test] fn onchain_inbox_reads_reference_rpc_tx() {
    let f = mail();
    let to = f["to"].as_str().unwrap();
    let msgs = inbox::messages_from_transactions(&[f["rpcTx"].clone()], Some(to));
    assert_eq!(msgs.len(), 1);
    let m = &msgs[0];
    assert_eq!(m.id, "req-0001");
    assert_eq!(m.sender_wallet, f["from"].as_str().unwrap());
    assert_eq!(m.content, "Does the billing webhook retry on 5xx?");
    assert_eq!(m.signature, f["signature"].as_str().unwrap());
    assert_eq!(m.transport, "solana-memo");
    assert!(inbox::messages_from_transactions(&[f["rpcTx"].clone()], Some(f["from"].as_str().unwrap())).is_empty());
}

#[test] fn envelope_rules_and_chunking() {
    assert!(Envelope::new("x", "a", "b", "bogus", "hi", Some(1)).is_err());
    assert!(Envelope::new("x", "a", "b", "request", &"z".repeat(8001), Some(1)).is_err());
    let long = Envelope::new("long-1", "a", "b", "event", &"héllo ✓ ".repeat(300), Some(1)).unwrap();
    let memos = envelope::encode_memos(&long).unwrap();
    assert!(memos.len() > 1 && memos.iter().all(|m| m.len() <= envelope::MEMO_MAX_BYTES));
    assert!(memos[0].starts_with("x402m/1>long-1 1/"));
    let mut asm = MemoAssembler::default();
    let mut got = None;
    for m in memos.iter().rev() { if let Some(e) = asm.feed("W", m) { got = Some(e); } }
    assert_eq!(got.unwrap(), long);
    assert!(asm.feed("W", &memos[0]).is_none(), "duplicate suppressed");
    let short = Envelope::new("s", "a", "b", "request", "hi", Some(1)).unwrap();
    assert_eq!(envelope::encode_memos(&short).unwrap(), vec![format!("x402m/1 {}", short.serialize())]);
}

#[test] fn card_is_x402m_v1_compatible() {
    let w = "GmaDrppBC7P5ARKV8g3djiwP89vz1jLK23V2GBjuAEGB";
    // exactly what x402-messaging solana/manifest.mjs buildNetworkCard emits
    let js = json!({"x402mCard":"v1","agentId":"agent-alice","wallet":w,"transports":["https-mailbox","solana-memo"],
        "endpoints":{"mailbox":"https://musebook.trade/api/auth/capability/execute","discovery":"https://musebook.trade/api/x402m/agents"},
        "handle":"alice","name":"Alice","reason":"alice/api-claude"});
    let c = card::parse(&js).unwrap();
    assert_eq!(c.network, "solana-devnet");
    let ours = serde_json::to_value(card::build(card::CardInput { agent_id: "a", wallet: w, ..Default::default() }, "https://e").unwrap()).unwrap();
    assert_eq!(ours["x402mCard"], "v1");
    assert_eq!(ours["endpoints"]["mailbox"], "https://musebook.trade/api/auth/capability/execute");
    assert_eq!(ours["transports"], json!(["https-mailbox","solana-memo"]));
    let r = addressing::resolve("@Alice", &[c.clone()], &[]).unwrap();
    assert_eq!(r.wallet, w);
    assert_eq!(addressing::resolve("alice/api-claude", &[c], &[]).unwrap().source, "address-book");
    let e = addressing::resolve("@bob", &[], &[json!({"agent_id":"agent-bob","handle":"bob"})]).unwrap_err();
    assert!(e.to_string().contains("no verified network card"));
}

#[test] fn capability_bodies_match_spec() {
    let b = client::capability_body("x402m.send", &json!({"to":"@peer","requestId":"r1","kind":"request","content":"hi"})).unwrap();
    assert_eq!(b, json!({"capability":"x402m.send","arguments":{"to":"@peer","requestId":"r1","kind":"request","content":"hi"}}));
    assert!(client::capability_body("x402m.send", &json!({"to":"@p","requestId":"r","kind":"request","content":"hi","from":"spoof"})).is_err());
    assert!(client::capability_body("x402m.send", &json!({"to":"@p","requestId":"r","kind":"nope","content":"hi"})).is_err());
    assert!(client::capability_body("x402m.register", &json!({"handle":"Bad Handle","name":"n"})).is_err());
    assert!(client::capability_body("x402m.inbox", &json!({})).is_ok());
    let j = client::jwt_claims("agent-1", "x402m.send", "{}", 100);
    assert_eq!(j["header"]["typ"], "agent+jwt");
    assert_eq!(j["claims"]["exp"], 160);
    assert_eq!(j["claims"]["ath"], "RBNvo1WzZ4oRRq0W9-hknpT7T8If536DEMBg9hyq_4o");
}

#[test] fn x402_payment_required_is_devnet_only_proposal() {
    let body = json!({"x402Version":1,"accepts":[
        {"scheme":"exact","network":"base-sepolia","maxAmountRequired":"10","payTo":"0xabc","asset":"0xusdc"},
        {"scheme":"exact","network":"solana-devnet","maxAmountRequired":"1000","payTo":"J2xccRtuG43drESLYznHhLhQkLTdfepcKYbiQ9BsJVaf","asset":"4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU","resource":"https://x/y"}]});
    let p = client::payment_proposal(&body).unwrap();
    assert_eq!(p["signed"], false);
    assert_eq!(p["pay"]["amount"], "1000");
    assert_eq!(p["planRequest"]["body"]["network"], "solana-devnet");
    let main = json!({"accepts":[{"scheme":"exact","network":"solana","maxAmountRequired":"1","payTo":"x"}]});
    assert!(client::payment_proposal(&main).unwrap_err().to_string().contains("devnet only"));
}
