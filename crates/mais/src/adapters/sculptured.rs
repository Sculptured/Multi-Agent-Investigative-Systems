//! A Sculptured run export (`sculptured.session/v1`): the header carries the team as it was at the
//! run's start; the events say which CLI session each agent ran, its handoffs and the plan readings.
use super::team::{Member, Team};
use crate::model::{BudgetEvent, BudgetEventKind, BudgetLimits, Edge, EdgeKind, PlanReading};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunExport {
    pub team: Team,
    pub started_ms: i64,
    pub ended_ms: Option<i64>,
    /// CLI session id → member id.
    pub sessions: BTreeMap<String, String>,
    /// Spawned child member → its parent member.
    pub parents: BTreeMap<String, String>,
    pub readings: Vec<PlanReading>,
    pub budget_events: Vec<BudgetEvent>,
}

/// `pod.member@rig` → `member`.
pub fn member_of(seat: &str) -> String {
    let s = seat.split('@').next().unwrap_or(seat);
    s.split_once('.').map_or(s, |(_, m)| m).to_string()
}

pub fn read(text: &str) -> Result<RunExport, String> {
    let mut lines = text.lines();
    let header: Value = serde_json::from_str(lines.next().ok_or("an empty file")?).map_err(|e| e.to_string())?;
    if header["format"] != "sculptured.session/v1" {
        return Err("not a sculptured.session/v1 file".into());
    }
    let mut x = RunExport {
        team: team_of(&header),
        started_ms: header["started_ms"].as_i64().unwrap_or(0),
        ended_ms: header["ended_ms"].as_i64(),
        ..Default::default()
    };
    for a in header["agents"].as_array().into_iter().flatten() {
        if let (Some(id), Some(p)) = (a["id"].as_str(), a["parent"].as_str()) {
            x.parents.insert(member_of(id), member_of(p));
        }
    }
    for line in lines {
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            event(&mut x, &v["event"]);
        }
    }
    Ok(x)
}

fn event(x: &mut RunExport, e: &Value) {
    let seat = e["subject"]["id"].as_str().unwrap_or_default();
    let at = e["at_ms"].as_i64().unwrap_or(0);
    let b = &e["body"];
    match b["type"].as_str().unwrap_or("") {
        "tenure-started" => {
            if let Some(s) = b["native_session_id"].as_str() {
                x.sessions.insert(s.to_string(), member_of(seat));
            }
        }
        "plan-usage-reported" => x.readings.extend(readings(&b["usage"], at, seat)),
        "handed-over" => {
            for f in b["from"].as_array().into_iter().flatten() {
                x.team.edges.push(Edge {
                    from: member_of(f["seat"].as_str().unwrap_or_default()),
                    to: member_of(seat),
                    kind: EdgeKind::HandsWorkTo,
                    at_ms: Some(at),
                });
            }
        }
        "budget-state-changed" => {
            let kind = if b["state"] == "stopped" { BudgetEventKind::Stop } else { BudgetEventKind::WrapUp };
            x.budget_events.push(BudgetEvent {
                kind,
                at_ms: at,
                reason: b["reason"].as_str().unwrap_or_default().to_string(),
            });
        }
        _ => {}
    }
}

fn readings(u: &Value, at: i64, seat: &str) -> Vec<PlanReading> {
    let provider = u["provider"].as_str().unwrap_or_default();
    u["windows"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|w| {
            Some(PlanReading {
                provider: provider.to_string(),
                window: w["name"].as_str()?.to_string(),
                percent: w["used_percent"].as_f64()?,
                resets_at: w["resets_at"].as_i64(),
                at_ms: at,
                reporter: Some(member_of(seat)),
            })
        })
        .collect()
}

fn list(v: &Value) -> impl Iterator<Item = &Value> {
    v.as_array().into_iter().flatten()
}

fn members(d: &Value) -> Vec<Member> {
    let groups: BTreeMap<&str, &str> =
        list(&d["groups"]).filter_map(|g| Some((g["id"].as_str()?, g["label"].as_str()?))).collect();
    list(&d["agents"])
        .filter_map(|a| {
            Some(Member {
                id: a["id"].as_str()?.to_string(),
                name: a["name"].as_str()?.to_string(),
                group: a["group_id"].as_str().and_then(|g| groups.get(g)).map(|s| s.to_string()),
            })
        })
        .collect()
}

fn edges(d: &Value) -> Vec<Edge> {
    list(&d["connections"])
        .filter(|c| c["kind"] == "hands-work-to")
        .filter_map(|c| {
            let (from, to) = (c["from"].as_str()?.to_string(), c["to"].as_str()?.to_string());
            Some(Edge { from, to, kind: EdgeKind::HandsWorkTo, at_ms: None })
        })
        .collect()
}

/// The team in the header (`sculptured.team/v1`).
fn team_of(h: &Value) -> Team {
    let d = &h["team"]["team"];
    Team {
        name: d["name"].as_str().map(String::from),
        members: members(d),
        edges: edges(d),
        budget: budget_of(&d["budget"]),
    }
}

fn budget_of(b: &Value) -> Option<BudgetLimits> {
    if !b.is_object() {
        return None;
    }
    let pair = |v: &Value| v["hard"].as_f64().map(|h| (v["soft"].as_f64().unwrap_or(h * 0.85), h));
    Some(BudgetLimits {
        plan: b["plan"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| {
                let (s, h) = pair(p)?;
                Some((p["provider"].as_str()?.to_string(), p["window"].as_str()?.to_string(), s, h))
            })
            .collect(),
        tokens: pair(&b["tokens"]).map(|(s, h)| (s as u64, h as u64)),
        minutes: pair(&b["minutes"]).map(|(s, h)| (s as u32, h as u32)),
        max_working: b["max_working"].as_u64().map(|n| n as u32),
    })
}
