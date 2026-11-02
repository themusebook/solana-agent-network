use solana_agent_network::helius::*;
#[test] fn routes_devnet_only() {
    assert!(route("https://api.devnet.solana.com", true).is_err());
    assert!(route("https://api.mainnet-beta.solana.com", false).is_err());
    assert_eq!(route("https://api.devnet.solana.com", false).unwrap(), Route::DevnetRpc { rpc: "https://api.devnet.solana.com".into() });
}
#[test] fn fee_math() {
    assert_eq!(percentile(vec![], 0.75), 0);
    assert_eq!(percentile(vec![5, 1, 9, 3], 0.75), 5);
    assert_eq!(set_cu_price_data(1000), [3, 232, 3, 0, 0, 0, 0, 0, 0]);
    assert_eq!(set_cu_limit_data(200_000)[0], 2);
}
#[test] fn caps_declared() { let c = capabilities(); assert_eq!(c["skills"].as_array().unwrap().len(), 6); assert_eq!(c["feeRouting"]["sender"]["enabled"], false); }
#[test] #[ignore] fn live_devnet_fee() { let (f, src) = priority_fee("https://api.devnet.solana.com", &["MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr"]).unwrap(); println!("{src} {f}"); }
