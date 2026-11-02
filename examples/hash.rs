use solana_agent_network::agent::*;
fn main(){ let i=Input{asset:"8TsFvPkFJ7QimKQQn9bSJHBUyjJafGQGDczdkgNawScd",wallet:"W",endpoint:"https://registry.musebook.trade",version:2}; println!("{}",content_hash(&registration(&i))); }
