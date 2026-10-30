use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::{json, Value};
use solana_agent_network::{card, config::*, daemon, mcp, service::Service, store::State};
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "san", version, about = "Solana Agent Network (devnet) CLI, daemon and MCP server")]
struct Cli {
    /// SAN endpoint (hosted daemon). Falls back to the registry. Env: SAN_ENDPOINT
    #[arg(long, global = true)] endpoint: Option<String>,
    #[command(subcommand)] cmd: Cmd,
}

#[derive(Subcommand)]
enum CardCmd {
    /// Build (and store locally) this node's public network card
    New { #[arg(long)] agent_id: String, #[arg(long)] wallet: String, #[arg(long)] asset: Option<String>,
          #[arg(long)] handle: Option<String>, #[arg(long)] name: Option<String> },
    /// Print the UNSIGNED Arweave (ANS-104/Turbo) upload payload for your card; the owner signs it
    Archive,
    /// Fetch an archived card by Arweave tx id via the gateway
    Fetch { txid: String },
    /// Find archived cards via gateway GraphQL (optionally by wallet)
    Search { #[arg(long)] wallet: Option<String> },
}

#[derive(Subcommand)]
enum Cmd {
    /// Print the one-line join prompt
    Prompt,
    /// Network manifest
    Manifest,
    /// Live joined-agent count
    Stats,
    /// List/discover joined nodes
    Nodes,
    /// Owner join link; with --asset and --authority, the UNSIGNED register_node tx for the owner's wallet
    Join { #[arg(long)] asset: Option<String>, #[arg(long)] authority: Option<String> },
    /// Verify an asset's onchain registration
    Verify { asset: String },
    /// Network cards: build, archive to Arweave (unsigned payload), fetch from Arweave
    #[command(subcommand)] Card(CardCmd),
    /// x402m mailbox operation against local state: register|send|inbox|ack with JSON args
    Mail { op: String, #[arg(default_value = "{}")] args: String },
    /// Run the MCP server on stdio
    Mcp,
    /// Run the daemon (HTTP API + MCP at /mcp + poller)
    Daemon { #[arg(long, default_value = "127.0.0.1:8787")] bind: String, #[arg(long, default_value_t = 30)] poll: u64,
             /// Hosted mode: read registry directly (this daemon IS the endpoint)
             #[arg(long)] hosted: bool },
    /// Install the daemon as a user service (launchd/systemd)
    Install { #[arg(long, default_value = "127.0.0.1:8787")] bind: String },
    /// Remove the user service file
    Uninstall,
}

fn print(v: &Value) { println!("{}", serde_json::to_string_pretty(v).unwrap()); }

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut cfg = Config::from_env()?;
    if let Some(e) = cli.endpoint { cfg.endpoint = e; }
    let primary = cfg.endpoint.clone();
    let svc = |p: &str, cfg: &Config| -> Result<Service> { Ok(Service::new(cfg.clone(), p, State::load()?, true)) };
    match cli.cmd {
        Cmd::Prompt => println!("{JOIN_PROMPT}"),
        Cmd::Manifest => print(&svc(&primary, &cfg)?.manifest()),
        Cmd::Stats => print(&svc(&primary, &cfg)?.stats()?),
        Cmd::Nodes => print(&json!({"nodes": svc(&primary, &cfg)?.nodes()?})),
        Cmd::Join { asset, authority } => print(&svc(&primary, &cfg)?.join(asset.as_deref(), authority.as_deref())?),
        Cmd::Verify { asset } => print(&svc(&primary, &cfg)?.verify(&asset)?),
        Cmd::Card(CardCmd::New { agent_id, wallet, asset, handle, name }) => {
            let c = card::build(card::CardInput { agent_id: &agent_id, wallet: &wallet, asset: asset.as_deref(),
                handle: handle.as_deref(), name: name.as_deref(), mailbox: None }, &cfg.endpoint)?;
            let mut st = State::load()?;
            st.me = Some(c.clone());
            st.upsert_card(c.clone());
            st.save()?;
            print(&serde_json::to_value(c)?);
        }
        Cmd::Card(CardCmd::Archive) => {
            let me = State::load()?.me.ok_or_else(|| anyhow::anyhow!("no card yet: run `san card new` first"))?;
            print(&solana_agent_network::arweave::archive_payload(&me)?);
        }
        Cmd::Card(CardCmd::Fetch { txid }) => {
            let (c, id) = solana_agent_network::arweave::fetch_card(&solana_agent_network::arweave::gateway(), &txid)?;
            print(&json!({"txid": id, "card": c}));
        }
        Cmd::Card(CardCmd::Search { wallet }) => print(&json!({"cards": solana_agent_network::arweave::discover_cards(&solana_agent_network::arweave::gateway(), wallet.as_deref())?})),
        Cmd::Mail { op, args } => print(&svc(&primary, &cfg)?.mailbox(&op, &serde_json::from_str(&args)?)?),
        Cmd::Mcp => mcp::serve_stdio(&svc(&primary, &cfg)?)?,
        Cmd::Daemon { bind, poll, hosted } => {
            let p = if hosted { cfg.registry.clone() } else { primary };
            daemon::run(Arc::new(svc(&p, &cfg)?), &bind, poll)?
        }
        Cmd::Install { bind } => { let (p, c) = daemon::service::install(&bind)?; println!("wrote {}\nstart with: {c}", p.display()); }
        Cmd::Uninstall => println!("removed {}", daemon::service::uninstall()?.display()),
    }
    Ok(())
}
