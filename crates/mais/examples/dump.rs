//! Prints what a reader finds in one session file: `cargo run --example dump -- <file>`.
fn main() {
    let path = std::env::args().nth(1).expect("a session file");
    let p = std::path::Path::new(&path);
    let sessions = if path.contains("/.codex/") {
        vec![mais::adapters::codex::read(p).expect("readable")]
    } else {
        mais::adapters::claude::read(p).expect("readable")
    };
    for s in sessions {
        let a = s.agent.as_ref().unwrap();
        let total: u64 = s.calls.iter().map(|c| c.usage.total()).sum();
        let mut kinds = std::collections::BTreeMap::new();
        for i in s.items.iter().filter(|i| i.first_call == 0) {
            *kinds.entry(format!("{:?}", i.kind)).or_insert(0u64) += i.chars;
        }
        let u: mais::model::Usage = s.calls.iter().map(|c| c.usage).sum();
        println!("{:?} readings={}", u, s.readings.len());
        println!(
            "{} calls={} total={} skipped={} compactions={:?} start_chars={:?} cost={:?}",
            a.id,
            s.calls.len(),
            total,
            s.skipped,
            s.compactions,
            kinds,
            a.cli_cost_usd
        );
    }
}
