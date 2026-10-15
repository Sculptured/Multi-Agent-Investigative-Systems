//! Team structure: who is in the team, in which group, and who hands work to whom. From a MADL
//! document, or from a Sculptured run export's team.
use crate::model::{BudgetLimits, Edge, EdgeKind};
use std::collections::BTreeMap;

/// One member of the team.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Member {
    pub id: String,
    pub name: String,
    pub group: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Team {
    pub name: Option<String>,
    pub members: Vec<Member>,
    pub edges: Vec<Edge>,
    pub budget: Option<BudgetLimits>,
}

impl Team {
    pub fn member(&self, id: &str) -> Option<&Member> {
        self.members.iter().find(|m| m.id == id)
    }
}

/// A MADL document's agents, groups, hand-work-to connections and run budget.
pub fn from_madl(doc: &madl::Document) -> Team {
    let groups: BTreeMap<&str, String> = doc.groups.iter().map(|g| (g.id.as_str(), g.name.clone().unwrap_or_else(|| g.id.clone()))).collect();
    let members = doc.agents.iter().map(|a| Member {
        id: a.id.clone(),
        name: a.name.clone().unwrap_or_else(|| a.id.clone()),
        group: a.groups.first().map(|g| groups.get(g.as_str()).cloned().unwrap_or_else(|| g.clone())),
    }).collect();
    let edges = doc.connections.iter().filter(|c| matches!(c.kind, madl::model::ConnectionKind::Handoff | madl::model::ConnectionKind::Delegate))
        .map(|c| Edge { from: c.from.clone(), to: c.to.clone(), kind: EdgeKind::HandsWorkTo, at_ms: None }).collect();
    Team { name: Some(doc.system.name.clone()), members, edges, budget: doc.system.budget.as_ref().map(budget_of) }
}

fn budget_of(b: &madl::model::RunBudget) -> BudgetLimits {
    let soft = |s: Option<f64>, h: f64| s.unwrap_or(h * 0.85);
    BudgetLimits {
        plan: b.plan.iter().map(|p| (p.provider.clone(), p.window.clone(), soft(p.soft, p.hard), p.hard)).collect(),
        tokens: b.tokens.as_ref().map(|t| (t.soft().copied().unwrap_or(t.hard() * 85 / 100), *t.hard())),
        minutes: None,
        max_working: b.max_working,
    }
}
