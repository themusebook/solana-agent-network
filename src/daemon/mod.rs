//! SAN daemon (adapted from Reason Network's src/daemon): background service that keeps this
//! node's card published, polls/discovers joined nodes, holds the x402m mailbox and serves
//! HTTP API + MCP on one URL. Hosted, it is THE endpoint users connect to.
pub mod service;

use crate::{mcp, service::Service};
use anyhow::Result;
use serde_json::{json, Value};
use std::{io::Read, sync::Arc, time::Duration};
use tiny_http::{Header, Method, Request, Response, Server};

pub fn route(svc: &Service, method: &str, path: &str, body: &Value) -> (u16, Value) { route_with(svc, method, path, body, None) }

fn query<'a>(url: &'a str, k: &str) -> Option<&'a str> {
    url.split_once('?')?.1.split('&').find_map(|kv| kv.split_once('=').filter(|(a, _)| *a == k).map(|(_, v)| v))
}

/// `bearer`: a caller's own fresh agent JWT, forwarded once to the x402m execute endpoint; never stored or logged.
pub fn route_with(svc: &Service, method: &str, url: &str, body: &Value, bearer: Option<&str>) -> (u16, Value) {
    let path = url.split('?').next().unwrap_or("/").trim_end_matches('/');
    let r: anyhow::Result<Value> = (|| Ok(match (method, path) {
        ("GET", "") | ("GET", "/api/network/health") | ("GET", "/health") =>
            json!({"ok":true,"service":"san-daemon","network":crate::config::NETWORK,"programId":crate::config::PROGRAM_ID}),
        ("GET", "/.well-known/agent-network.json") | ("GET", "/api/network/manifest") => svc.manifest(),
        ("GET", "/api/network/stats") => svc.stats()?,
        ("GET", "/api/network/agents") | ("GET", "/api/san/nodes") => json!({"ok":true,"agents":svc.nodes()?}),
        ("GET", "/api/san/arweave/status") => crate::arweave::status(&crate::arweave::gateway(), &crate::arweave::ao_url()),
        ("POST", "/api/san/arweave/heartbeat") => crate::arweave::heartbeat_payload(body["agentId"].as_str().unwrap_or(""), body["wallet"].as_str().unwrap_or(""), body["nodePda"].as_str(), crate::store::now_ms())?,
        ("GET", "/api/san/cards") => json!({"ok":true,"cards":svc.cards()}),
        ("POST", "/api/san/cards") => json!({"ok":true,"card":svc.publish_card(body)?}),
        ("GET", "/api/san/join") | ("GET", "/network/join") => svc.join(None, None)?,
        ("POST", "/api/network/tx/register-node") | ("POST", "/api/san/join") =>
            svc.join(Some(body["asset"].as_str().unwrap_or("")), Some(body["authority"].as_str().unwrap_or("")))?,
        ("GET", "/api/x402m/directory") => json!({"ok":true,"agents":crate::x402m::client::fetch_directory()?}),
        ("POST", "/api/x402m/send") => svc.msg_send(body, bearer)?,
        ("POST", "/api/x402m/relay") => svc.msg_capability(body["capability"].as_str().unwrap_or(""), &body["arguments"], bearer)?,
        ("GET", "/api/x402m/onchain-inbox") => svc.msg_onchain_inbox(query(url, "wallet").unwrap_or(""), query(url, "limit").and_then(|l| l.parse().ok()).unwrap_or(25))?,
        ("POST", "/api/x402m/payment-proposal") => crate::x402m::client::payment_proposal(body)?,
        ("POST", "/mcp") => return Ok(mcp::handle(svc, body).unwrap_or(json!({}))),
        ("POST", p) if p.starts_with("/api/san/mailbox/") => svc.mailbox(&p["/api/san/mailbox/".len()..], body)?,
        ("GET", p) if p.starts_with("/api/network/verify/") => svc.verify(&p["/api/network/verify/".len()..])?,
        _ => return Err(anyhow::anyhow!("not_found")),
    }))();
    match r {
        Ok(v) => (200, v),
        Err(e) if e.to_string() == "not_found" => (404, json!({"ok":false,"error":"not_found"})),
        Err(e) => (400, json!({"ok":false,"error":e.to_string()})),
    }
}

fn respond(svc: &Service, mut req: Request) {
    let mut buf = String::new();
    let _ = req.as_reader().take(64 * 1024).read_to_string(&mut buf);
    let body: Value = serde_json::from_str(&buf).unwrap_or(Value::Null);
    let (code, v) = if *req.method() == Method::Options { (204, Value::Null) } else { { let bearer = req.headers().iter().find(|h| h.field.equiv("authorization")).and_then(|h| h.value.as_str().strip_prefix("Bearer ").map(String::from)); route_with(svc, req.method().as_str(), req.url(), &body, bearer.as_deref()) } };
    let h = |k: &str, v: &str| Header::from_bytes(k, v).unwrap();
    let resp = Response::from_string(if v.is_null() { String::new() } else { v.to_string() }).with_status_code(code)
        .with_header(h("content-type", "application/json"))
        .with_header(h("access-control-allow-origin", "*"))
        .with_header(h("access-control-allow-headers", "content-type, authorization"))
        .with_header(h("access-control-allow-methods", "GET, POST, OPTIONS"));
    let _ = req.respond(resp);
}

/// Run the daemon: poller thread + HTTP server.
pub fn run(svc: Arc<Service>, bind: &str, poll_secs: u64) -> Result<()> {
    let poller = svc.clone();
    std::thread::spawn(move || loop {
        if let Err(e) = poller.refresh() { eprintln!("san-daemon: refresh failed: {e}"); }
        std::thread::sleep(Duration::from_secs(poll_secs.max(5)));
    });
    let server = Server::http(bind).map_err(|e| anyhow::anyhow!("bind {bind}: {e}"))?;
    eprintln!("san-daemon listening on http://{bind} (devnet, program {})", crate::config::PROGRAM_ID);
    for req in server.incoming_requests() {
        let s = svc.clone();
        std::thread::spawn(move || respond(&s, req));
    }
    Ok(())
}
