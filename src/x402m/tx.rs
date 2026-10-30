//! UNSIGNED on-chain mail transaction (port of solana/transaction.mjs). Never signs.
use super::envelope::{encode_memos, Envelope};
use anyhow::{bail, Result};
use base64::Engine;
use serde::Serialize;

pub const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";
pub const MEMO_PROGRAM: &str = "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MailTx { pub transaction_base64: String, pub message_id: String, pub memos: Vec<String>, pub lamports: u64, pub signed: bool }

fn key(s: &str, what: &str) -> Result<[u8; 32]> {
    match bs58::decode(s).into_vec() { Ok(v) if v.len() == 32 => Ok(v.try_into().unwrap()), _ => bail!("{what} must be a base58 Solana address") }
}
fn shortvec(mut v: usize, out: &mut Vec<u8>) {
    loop { let b = (v & 0x7f) as u8; v >>= 7; if v == 0 { out.push(b); return; } out.push(b | 0x80); }
}

pub fn build_mail_transaction(from_wallet: &str, to_wallet: &str, e: &Envelope, recent_blockhash: &str) -> Result<MailTx> {
    e.validate()?;
    let from = key(from_wallet, "fromWallet and toWallet")?;
    let to = key(to_wallet, "fromWallet and toWallet")?;
    let bh = match bs58::decode(recent_blockhash).into_vec() { Ok(v) if v.len() == 32 => v, _ => bail!("recentBlockhash must be a base58 blockhash") };
    let sys = key(SYSTEM_PROGRAM, "system")?; let memo = key(MEMO_PROGRAM, "memo")?;
    let memos = encode_memos(e)?;
    // accounts: payer (signer,w), recipient (w), system (ro), memo (ro) — same order as web3.js for this shape
    let mut accounts: Vec<[u8; 32]> = vec![from];
    if to != from { accounts.push(to); }
    accounts.push(sys); accounts.push(memo);
    let idx = |k: &[u8; 32]| accounts.iter().position(|a| a == k).unwrap() as u8;
    let mut m = vec![1u8, 0, 2];
    shortvec(accounts.len(), &mut m);
    for a in &accounts { m.extend_from_slice(a); }
    m.extend_from_slice(&bh);
    shortvec(1 + memos.len(), &mut m);
    // System transfer of 0 lamports
    let mut data = 2u32.to_le_bytes().to_vec(); data.extend_from_slice(&0u64.to_le_bytes());
    m.push(idx(&sys)); shortvec(2, &mut m); m.push(idx(&from)); m.push(idx(&to)); shortvec(data.len(), &mut m); m.extend(data);
    for s in &memos { m.push(idx(&memo)); shortvec(0, &mut m); shortvec(s.len(), &mut m); m.extend_from_slice(s.as_bytes()); }
    let mut wire = vec![]; shortvec(1, &mut wire); wire.extend([0u8; 64]); wire.extend(m);
    Ok(MailTx { transaction_base64: base64::engine::general_purpose::STANDARD.encode(wire), message_id: e.id.clone(), memos, lamports: 0, signed: false })
}
