//! Prints what the Claude Code reader finds in a session file: `cargo run --example claude_dump -- <file>`.
fn main() {
    let path = std::env::args().nth(1).expect("a session file");
    for s in mais::adapters::claude::read(std::path::Path::new(&path)).expect("readable") {
        let a = s.agent.as_ref().unwrap();
        let total: u64 = s.calls.iter().map(|c| c.usage.total()).sum();
        let mut kinds = std::collections::BTreeMap::new();
        for i in s.items.iter().filter(|i| i.first_call == 0) {
            *kinds.entry(format!("{:?}", i.kind)).or_insert(0u64) += i.chars;
        }
        println!("{} calls={} total={} skipped={} compactions={:?} start_chars={:?} cost={:?}", a.id, s.calls.len(), total, s.skipped, s.compactions, kinds, a.cli_cost_usd);
    }
}
