use serde_json::json;
use solana_agent_network::modules::{self, perps, pumpfun};
use solana_agent_network::modules::Module;
#[test] fn nothing_executes() { for m in modules::all() { assert!(m.execute(&json!({})).is_err(), "{}", m.id()); } }
#[test] fn imperial_flag_refused() {
    std::env::set_var("IMPERIAL_TRADING_ENABLED", "true");
    assert!(perps::Perps::imperial().execute(&json!({})).unwrap_err().to_string().contains("refused"));
    std::env::set_var("IMPERIAL_TRADING_ENABLED", "false");
}
#[test] fn pump_quote() {
    // initial curve values documented in pump-public-docs PUMP_PROGRAM_README (Global example)
    let (out, fee) = pumpfun::quote_buy(30_000_000_000, 1_073_000_000_000_000, 793_100_000_000_000, 1_000_000_000, 95, 5).unwrap();
    assert_eq!(fee, 10_000_000);
    assert!(out > 0 && out < 1_073_000_000_000_000);
    let s = modules::pumpfun::PumpFun;
    assert_eq!(s.simulate(&json!({"virtualSolReserves":30_000_000_000u64,"virtualTokenReserves":1_073_000_000_000_000u64,"realTokenReserves":793_100_000_000_000u64,"solIn":1_000_000_000u64,"feeBasisPoints":95,"creatorFeeBasisPoints":5})).unwrap()["tokensOut"], out);
}
#[test] fn perps_liq() { assert!((perps::liq_price(100.0, 10.0, 0.05, true).unwrap() - 95.0).abs() < 1e-9); assert!((perps::liq_price(100.0, 10.0, 0.05, false).unwrap() - 105.0).abs() < 1e-9); }
#[test] fn ore_share() { assert!((modules::ore::expected_share(1.0, 9.0, 10.0, 0.5).unwrap() - 0.5).abs() < 1e-9); }
