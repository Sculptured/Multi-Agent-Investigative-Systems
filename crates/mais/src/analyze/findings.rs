//! The report's sentences: each states one thing a person can act on and names its section.
use crate::report::{Finding, Report};

fn pct(x: f64) -> String {
    format!("{:.0}%", x * 100.0)
}

fn tokens(n: u64) -> String {
    match n {
        n if n >= 1_000_000 => format!("{:.1}M", n as f64 / 1e6),
        n if n >= 1_000 => format!("{:.0}k", n as f64 / 1e3),
        n => n.to_string(),
    }
}

fn say(out: &mut Vec<Finding>, section: &str, text: String) {
    out.push(Finding { text, section: section.into() });
}

pub fn findings(r: &Report) -> Vec<Finding> {
    let mut out = Vec::new();
    let total = r.run.tokens.max(1) as f64;
    let start: u64 = r.where_tokens_went.iter().filter(|p| p.group == "start").map(|p| p.tokens).sum();
    if start > 0 {
        say(&mut out, "where_tokens_went", format!("{} of all tokens were agents re-reading their starting context on every call.", pct(start as f64 / total)));
    }
    if let Some(setup) = r.where_tokens_went.iter().find(|p| p.part.starts_with("Your own CLI setup")) {
        say(&mut out, "where_tokens_went", format!("Your own CLI setup (skills list, plugins, hooks) was {} of the starting context and {} of all tokens (estimated). Agents rarely need it: leave it out.", pct(setup.tokens as f64 / start.max(1) as f64), pct(setup.tokens as f64 / total)));
    }
    if let Some(g) = r.where_tokens_went.iter().find(|p| p.group == "growth") {
        say(&mut out, "where_tokens_went", format!("The largest cause of growth was {}: {} of all tokens, re-read on every later call.", g.part.to_lowercase(), pct(g.tokens as f64 / total)));
    }
    if let Some(f) = r.timeline.fanouts.iter().max_by_key(|f| f.agents) {
        let by = f.from.as_deref().map(|a| format!(" after {a} handed out work")).unwrap_or_default();
        say(&mut out, "timeline", format!("{} agents started within two minutes{by}; up to {} worked at once.", f.agents, r.run.most_working));
    }
    if let Some(s) = r.shared_reads.first().filter(|s| s.agents >= 3) {
        say(&mut out, "shared_reads", format!("{} agents each read {} into their own context.", s.agents, s.target));
    }
    if let Some(w) = r.shared_start.writes_avoidable.value.filter(|w| *w > 0) {
        say(&mut out, "shared_start", format!("{} tokens of each agent's start were identical across agents, but every agent wrote its own copy to the cache: about {} of cache writes could have been reads with the shared part first (estimated).", tokens(r.shared_start.identical_per_agent.value.unwrap_or(0)), tokens(w)));
    }
    let unused: Vec<&crate::report::ToolUse> = r.tools.iter().filter(|t| t.calls == 0).collect();
    let unused_reread: u64 = unused.iter().filter_map(|t| t.definition_reread.value).sum();
    if !unused.is_empty() && unused_reread > 0 {
        say(&mut out, "tools", format!("{} tools were given and never called; their definitions were re-read for {} tokens (estimated).", unused.len(), tokens(unused_reread)));
    }
    for p in r.plan.iter().filter(|p| p.added.value.is_some_and(|a| a > 0.0)) {
        let per = p.points_per_million.value.map(|v| format!(", about {v:.1} points per million tokens")).unwrap_or_default();
        say(&mut out, "plan", format!("The run added {:.0} points to the {} {} window ({:.0}% → {:.0}%){per}.", p.added.value.unwrap_or(0.0), p.provider, p.window.replace('_', "-"), p.first, p.highest));
    }
    if let (Some(m), Some(w)) = (r.latency.model_share.value, r.latency.waiting_share.value) {
        let t = r.latency.tool_share.value.unwrap_or(0.0);
        say(&mut out, "latency", format!("Agents spent {} of their active time waiting on the model, {} in tools and {} waiting for work or teammates.", pct(m), pct(t), pct(w)));
    }
    out
}
