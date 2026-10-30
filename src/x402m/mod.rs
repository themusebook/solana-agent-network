//! x402m/1 messaging, ported from github.com/solizardking/x402-messaging
//! (MIT License, Copyright (c) 2026 Musebook). See NOTICE.
//!
//! - `envelope`: x402m/1 envelope + SPL Memo wire format (single and chunked) and reassembly
//! - `tx`: UNSIGNED on-chain mail transaction (0-lamport transfer + memos), byte-identical to the JS reference
//! - `inbox`: read on-chain mail from getTransaction results / a devnet wallet's history
//! - `addressing`: resolve wallet / @handle / agent id through verified network cards
//! - `client`: HTTPS mailbox capability bodies (x402m.register/send/inbox/ack/link), agent-JWT claims,
//!   and x402 `402 Payment Required` handling (unsigned proposals only)
//!
//! Nothing here signs, pays or holds keys.
pub mod addressing;
pub mod client;
pub mod envelope;
pub mod inbox;
pub mod tx;

pub const DISCOVERY_URL: &str = "https://musebook.trade/api/x402m/discovery";
pub const AGENTS_URL: &str = "https://musebook.trade/api/x402m/agents";
pub const EXECUTE_URL: &str = "https://musebook.trade/api/auth/capability/execute";
