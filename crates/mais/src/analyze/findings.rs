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

fn found(section: &str, text: String) -> Option<Finding> {
    Some(Finding { text, section: section.into() })
}

fn share(part: u64, of: u64) -> String {
    pct(part as f64 / of.max(1) as f64)
}

fn start_tokens(r: &Report) -> u64 {
    r.where_tokens_went.iter().filter(|p| p.group == "start").map(|p| p.tokens).sum()
}

fn start(r: &Report) -> Option<Finding> {
    let start = start_tokens(r);
    let text = format!(
        "{} of all tokens were agents re-reading their starting context on every call.",
        share(start, r.run.tokens)
    );
    if start > 0 { found("where_tokens_went", text) } else { None }
}

fn setup(r: &Report) -> Option<Finding> {
    let setup = r.where_tokens_went.iter().find(|p| p.part.starts_with("Your own CLI setup"))?;
    let (of_start, of_all) = (share(setup.tokens, start_tokens(r)), share(setup.tokens, r.run.tokens));
    found(
        "where_tokens_went",
        format!(
            "Your own CLI setup (skills list, plugins, hooks) was {of_start} of the starting context and {of_all} of all \
             tokens (estimated). Agents rarely need it: leave it out."
        ),
    )
}

fn growth(r: &Report) -> Option<Finding> {
    let g = r.where_tokens_went.iter().find(|p| p.group == "growth")?;
    let (cause, of_all) = (g.part.to_lowercase(), share(g.tokens, r.run.tokens));
    found(
        "where_tokens_went",
        format!("The largest cause of growth was {cause}: {of_all} of all tokens, re-read on every later call."),
    )
}

fn fanout(r: &Report) -> Option<Finding> {
    let f = r.timeline.fanouts.iter().max_by_key(|f| f.agents)?;
    let name = |id: &str| r.agents.iter().find(|a| a.id == id).map_or(id.to_string(), |a| a.name.clone());
    let by = f.from.as_deref().map(|a| format!(" after {} handed out work", name(a))).unwrap_or_default();
    found(
        "timeline",
        format!("{} agents started within two minutes{by}; up to {} worked at once.", f.agents, r.run.most_working),
    )
}

fn shared_read(r: &Report) -> Option<Finding> {
    let s = r.shared_reads.first().filter(|s| s.agents >= 3)?;
    found("shared_reads", format!("{} agents each read {} into their own context.", s.agents, s.target))
}

fn shared_start(r: &Report) -> Option<Finding> {
    let w = r.shared_start.writes_avoidable.value.filter(|w| *w > 0)?;
    let same = tokens(r.shared_start.identical_per_agent.value.unwrap_or(0));
    found(
        "shared_start",
        format!(
            "{same} tokens of each agent's start were identical across agents, but every agent wrote its own copy to \
             the cache: about {} of cache writes could have been reads with the shared part first (estimated).",
            tokens(w)
        ),
    )
}

fn unused_tools(r: &Report) -> Option<Finding> {
    let unused: Vec<&crate::report::ToolUse> = r.tools.iter().filter(|t| t.calls == 0).collect();
    let reread: u64 = unused.iter().filter_map(|t| t.definition_reread.value).sum();
    let text = format!(
        "{} tools were given and never called; their definitions were re-read for {} tokens (estimated).",
        unused.len(),
        tokens(reread)
    );
    if reread > 0 { found("tools", text) } else { None }
}

fn plan(p: &crate::report::PlanSeries) -> Option<Finding> {
    let added = p.added.value.filter(|a| *a > 0.0)?;
    let per =
        p.points_per_million.value.map(|v| format!(", about {v:.1} points per million tokens")).unwrap_or_default();
    let window = p.window.replace('_', "-");
    found(
        "plan",
        format!(
            "The run added {added:.0} points to the {} {window} window ({:.0}% → {:.0}%){per}.",
            p.provider, p.first, p.highest
        ),
    )
}

fn latency(r: &Report) -> Option<Finding> {
    let l = &r.latency;
    let (m, w) = (l.model_share.value?, l.waiting_share.value?);
    let t = l.tool_share.value.unwrap_or(0.0);
    found(
        "latency",
        format!(
            "Agents spent {} of their active time waiting on the model, {} in tools and {} waiting for work or teammates.",
            pct(m),
            pct(t),
            pct(w)
        ),
    )
}

pub fn findings(r: &Report) -> Vec<Finding> {
    let first = [start(r), setup(r), growth(r), fanout(r), shared_read(r), shared_start(r), unused_tools(r)];
    let plans = r.plan.iter().map(plan);
    first.into_iter().chain(plans).chain([latency(r)]).flatten().collect()
}
