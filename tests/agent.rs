use solana_agent_network::agent::*;
const A: &str = "8TsFvPkFJ7QimKQQn9bSJHBUyjJafGQGDczdkgNawScd";
fn i(v: u32) -> Input<'static> { Input { asset: A, wallet: "BkSaL9JDAwhPG9C5fz3hpcjtGQwbXGSfuSvazTwRZhzC", endpoint: "https://registry.musebook.trade", version: v } }
#[test] fn registration_is_erc8004_x402_a2a() {
    let r = registration(&i(1));
    assert_eq!(r["name"], "solana-agent");
    assert!(r["type"].as_str().unwrap().contains("eip-8004"));
    assert_eq!(r["x402"]["network"], "solana-devnet");
    assert!(r["services"].as_array().unwrap().iter().any(|s| s["name"] == "A2A"));
    for m in default_modules() { if m.kind == "adapter" { assert!(!m.enabled) } }
    assert_eq!(a2a_card(&i(1))["name"], "solana-agent");
}
#[test] fn anchor_roundtrip_and_versioning() {
    let h1 = content_hash(&registration(&i(1))); let h2 = content_hash(&registration(&i(2)));
    assert_ne!(h1, h2);
    let (a, v, h) = parse_anchor(&anchor_memo(A, 2, &h2)).unwrap();
    assert_eq!((a.as_str(), v, h), (A, 2, h2));
    assert!(parse_anchor("san1:reg:x:1:short").is_err());
}
#[test] fn delegation_is_scoped() {
    let d = Delegation { asset: A.into(), delegate: "D".into(), owner_at_grant: "O".into(), programs: vec!["P".into()], spend_cap_lamports: 100, spent_lamports: 0, expires_at: 1000, nonce: 1 };
    assert!(d.check("O", "P", 50, 10, 0).is_ok());
    assert!(d.check("X", "P", 50, 10, 0).is_err());
    assert!(d.check("O", "Q", 50, 10, 0).is_err());
    assert!(d.check("O", "P", 500, 10, 0).is_err());
    assert!(d.check("O", "P", 50, 1000, 0).is_err());
    assert!(d.check("O", "P", 50, 10, 2).is_err());
}
