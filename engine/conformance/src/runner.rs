// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Executes one conformance vector against `fc4-core`. See README.md for the
//! format. Any deviation is reported as `Err(String)` naming the step.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use fc4_core::host::{AgentDelivery, Broadcast, Clock, TimerSource};
use fc4_core::ids::{cheat, msg, LEVEL_COUNT};
use fc4_core::julian;
use fc4_core::{AgentHandle, Message, Simulator, Snapshot, Tuning, Turbo, TURBO_DEFAULT_FRAME_MS};

use crate::json::Json;

type R<T> = Result<T, String>;

/// Format version understood by this runner.
pub const FORMAT_VERSION: i64 = 1;

/// Safety net against runaway loops (e.g. a frozen clock in OnTick).
const MAX_CLOCK_READS_PER_STEP: u64 = 50_000_000;

// ---- events -------------------------------------------------------------------

#[derive(Clone, Debug)]
pub(crate) enum Event {
    Deliver {
        agent: String,
        msg: Message,
        t: u32,
    },
    Post {
        msg: Message,
        t: u32,
    },
    AddRef {
        agent: String,
    },
    Release {
        agent: String,
    },
    Publish {
        cycles_to_ms: f32,
        ms_to_cycles: f32,
    },
    /// A budget post (kind "post"), with the labels of its void1 / IGZUnknown objects.
    BudgetPost {
        msg: Message,
        void1: String,
        unknown: String,
    },
    History {
        series: u32,
        value: i32,
    },
    HistoryQueue {
        series: u32,
        length: u32,
    },
    ShowBudgetWindow {
        panel: u32,
    },
    Disaster {
        id: u32,
        x: f32,
        z: f32,
    },
}

impl Event {
    fn to_json(&self) -> String {
        let m = |m: &Message| {
            format!(
                "\"type\":\"{}\",\"data1\":{},\"data2\":{},\"data3\":{}",
                type_name(m.msg_type),
                m.data1,
                m.data2,
                m.data3
            )
        };
        match self {
            Event::Deliver { agent, msg, t } => {
                format!("{{\"event\":\"deliver\",\"agent\":\"{agent}\",{},\"t\":{t}}}", m(msg))
            }
            Event::Post { msg, t } => format!("{{\"event\":\"post\",{},\"t\":{t}}}", m(msg)),
            Event::AddRef { agent } => format!("{{\"event\":\"add_ref\",\"agent\":\"{agent}\"}}"),
            Event::Release { agent } => format!("{{\"event\":\"release\",\"agent\":\"{agent}\"}}"),
            Event::Publish {
                cycles_to_ms,
                ms_to_cycles,
            } => {
                format!("{{\"event\":\"publish\",\"cycles_to_ms\":{cycles_to_ms:e},\"ms_to_cycles\":{ms_to_cycles:e}}}")
            }
            Event::BudgetPost { msg, void1, unknown } => {
                format!(
                    "{{\"event\":\"post\",{},\"void1\":\"{void1}\",\"unknown\":\"{unknown}\"}}",
                    m(msg)
                )
            }
            Event::History { series, value } => {
                format!("{{\"event\":\"history\",\"series\":\"0x{series:08X}\",\"value\":{value}}}")
            }
            Event::HistoryQueue { series, length } => {
                format!("{{\"event\":\"history_queue\",\"series\":\"0x{series:08X}\",\"length\":{length}}}")
            }
            Event::ShowBudgetWindow { panel } => format!("{{\"event\":\"show_budget_window\",\"panel\":{panel}}}"),
            Event::Disaster { id, x, z } => {
                format!("{{\"event\":\"disaster\",\"id\":\"0x{id:08X}\",\"x\":{x},\"z\":{z}}}")
            }
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Event::Deliver { .. } => "deliver",
            Event::Post { .. } => "post",
            Event::AddRef { .. } => "add_ref",
            Event::Release { .. } => "release",
            Event::Publish { .. } => "publish",
            Event::BudgetPost { .. } => "post",
            Event::History { .. } => "history",
            Event::HistoryQueue { .. } => "history_queue",
            Event::ShowBudgetWindow { .. } => "show_budget_window",
            Event::Disaster { .. } => "disaster",
        }
    }

    fn agent(&self) -> Option<&str> {
        match self {
            Event::Deliver { agent, .. } | Event::AddRef { agent } | Event::Release { agent } => Some(agent),
            _ => None,
        }
    }

    fn msg(&self) -> Option<&Message> {
        match self {
            Event::Deliver { msg, .. } | Event::Post { msg, .. } | Event::BudgetPost { msg, .. } => Some(msg),
            _ => None,
        }
    }

    fn t(&self) -> Option<u32> {
        match self {
            Event::Deliver { t, .. } | Event::Post { t, .. } => Some(*t),
            _ => None,
        }
    }
}

// ---- symbolic names -------------------------------------------------------------

const TYPE_NAMES: &[(&str, u32)] = &[
    ("SimBegin", msg::SIM_LEVEL_BASE),
    ("SimEmergency", msg::SIM_LEVEL_BASE + 1),
    ("SimNewMinute", msg::SIM_LEVEL_BASE + 2),
    ("SimNewHour", msg::SIM_LEVEL_BASE + 3),
    ("SimNewDay", msg::SIM_LEVEL_BASE + 4),
    ("SimNewWeek", msg::SIM_LEVEL_BASE + 5),
    ("SimNewMonth", msg::SIM_LEVEL_BASE + 6),
    ("SimNewYear", msg::SIM_LEVEL_BASE + 7),
    ("SimSpecificDay", msg::SIM_LEVEL_BASE + 8),
    ("SimSpecificTime", msg::SIM_LEVEL_BASE + 9),
    ("SimIdle", msg::SIM_LEVEL_BASE + 10),
    ("PauseChanged", msg::PAUSE_CHANGED),
    ("HiddenPauseChanged", msg::HIDDEN_PAUSE_CHANGED),
    ("EmergencyPauseChanged", msg::EMERGENCY_PAUSE_CHANGED),
    ("SpeedChanged", msg::SPEED_CHANGED),
    ("FundsChanged", 0x0772_FAD4),
    ("BondIssued", 0xC772_FADA),
    ("BondFinished", 0xCA3F_BDCE),
    ("EndGameForBudgetFailure", 0xAA5C_F1E3),
    ("BudgetGroupChanged", 0x0A3E_7624),
    ("FundingPercentageChanged", 0x08DA_C3E7),
    ("IdealMonthlyFundingChanged", 0x08DA_C3E8),
    ("BudgetForOccupantChanged", 0x6A5F_D3FD),
    ("CityEstablished", 0x26D3_1EC4),
    ("ToggleBudgetDialog", 0x2922_2F02),
];

fn type_name(t: u32) -> String {
    TYPE_NAMES
        .iter()
        .find(|(_, v)| *v == t)
        .map(|(n, _)| (*n).to_owned())
        .unwrap_or_else(|| format!("0x{t:08X}"))
}

pub(crate) fn parse_type(j: &Json) -> R<u32> {
    if let Some(s) = j.as_str() {
        if let Some((_, v)) = TYPE_NAMES.iter().find(|(n, _)| *n == s) {
            return Ok(*v);
        }
    }
    j.as_i64()
        .map(|v| v as u32)
        .ok_or_else(|| format!("bad message type {j:?}"))
}

fn parse_cheat_id(j: &Json) -> R<u32> {
    match j.as_str() {
        Some("FPS") => Ok(cheat::FPS),
        Some("SimDate") => Ok(cheat::SIM_DATE),
        _ => j
            .as_i64()
            .map(|v| v as u32)
            .ok_or_else(|| format!("bad cheat id {j:?}")),
    }
}

// ---- host --------------------------------------------------------------------------

#[derive(Clone, Default)]
struct Behaviour {
    /// Calls needed per interval; 0 = never finishes.
    finish_after: u32,
    cost_ms: u32,
    /// 1-based total call number -> operations run during that call.
    on_call: HashMap<u32, Vec<Json>>,
}

#[derive(Default)]
struct AgentRuntime {
    behaviour: Behaviour,
    total_calls: u32,
    pending_calls: u32,
}

pub(crate) struct HostState {
    now: Cell<u32>,
    auto_step: Cell<u32>,
    reads: Cell<u64>,
    pub(crate) events: RefCell<Vec<Event>>,
    agents: RefCell<HashMap<AgentHandle, AgentRuntime>>,
    labels: RefCell<HashMap<String, AgentHandle>>,
    names: RefCell<HashMap<AgentHandle, String>>,
    error: RefCell<Option<String>>,
    slots: RefCell<HashMap<String, Vec<u8>>>,
    qpc: Cell<u64>,
    qpc_frequency: Cell<u64>,
    tsc: Cell<u64>,
    /// The budget world (budget vectors), created on first use.
    pub(crate) budget: RefCell<Option<Rc<crate::budget::World>>>,
    /// Worlds of the scalar-simulator vectors (demand, ordinance, weather),
    /// by name, created on first use.
    pub(crate) ext: RefCell<HashMap<&'static str, Rc<dyn std::any::Any>>>,
    /// The named grids (simgrid vectors), created on first use.
    pub(crate) grids: RefCell<Option<Rc<crate::simgrid::World>>>,
    /// The land value world (land value vectors), created on first use.
    pub(crate) landvalue: RefCell<Option<Rc<crate::landvalue::World>>>,
}

impl HostState {
    fn label(&self, h: AgentHandle) -> String {
        self.names.borrow().get(&h).cloned().unwrap_or_else(|| format!("#{h}"))
    }

    fn handle(&self, label: &str) -> AgentHandle {
        if let Some(h) = self.labels.borrow().get(label) {
            return *h;
        }
        let h = 1000 + self.labels.borrow().len() as AgentHandle;
        self.labels.borrow_mut().insert(label.to_owned(), h);
        self.names.borrow_mut().insert(h, label.to_owned());
        h
    }

    fn fail(&self, e: String) {
        let mut slot = self.error.borrow_mut();
        if slot.is_none() {
            *slot = Some(e);
        }
    }
}

#[derive(Clone)]
pub(crate) struct HostRef(pub(crate) Rc<HostState>);

impl Clock for HostRef {
    fn now_ms(&self) -> u32 {
        let h = &self.0;
        let reads = h.reads.get() + 1;
        h.reads.set(reads);
        assert!(
            reads < MAX_CLOCK_READS_PER_STEP,
            "runaway loop: clock read {reads} times in one step"
        );
        let t = h.now.get();
        h.now.set(t.wrapping_add(h.auto_step.get()));
        t
    }
}

impl AgentDelivery for HostRef {
    fn deliver(&self, sim: &Simulator, agent: AgentHandle, m: &Message) -> bool {
        let h = &self.0;
        h.events.borrow_mut().push(Event::Deliver {
            agent: h.label(agent),
            msg: *m,
            t: h.now.get(),
        });
        let (finished, cost, ops) = {
            let mut agents = h.agents.borrow_mut();
            let rt = agents.entry(agent).or_insert_with(|| AgentRuntime {
                behaviour: Behaviour {
                    finish_after: 1,
                    ..Default::default()
                },
                ..Default::default()
            });
            rt.total_calls += 1;
            rt.pending_calls += 1;
            let finished = rt.behaviour.finish_after != 0 && rt.pending_calls >= rt.behaviour.finish_after;
            if finished {
                rt.pending_calls = 0;
            }
            (
                finished,
                rt.behaviour.cost_ms,
                rt.behaviour.on_call.get(&rt.total_calls).cloned(),
            )
        };
        h.now.set(h.now.get().wrapping_add(cost));
        if let Some(ops) = ops {
            for op in &ops {
                if let Err(e) = exec(sim, self, op) {
                    h.fail(format!("in on_call of agent {}: {e}", h.label(agent)));
                }
            }
        }
        finished
    }

    fn add_ref(&self, agent: AgentHandle) {
        let h = &self.0;
        h.events.borrow_mut().push(Event::AddRef { agent: h.label(agent) });
    }

    fn release(&self, agent: AgentHandle) {
        let h = &self.0;
        h.events.borrow_mut().push(Event::Release { agent: h.label(agent) });
    }
}

/// Fake wall clock / TSC for the limit-timer calibration (spec §5.5).
struct TimerRef(Rc<HostState>);

impl TimerSource for TimerRef {
    fn qpc(&self) -> u64 {
        self.0.qpc.get()
    }
    fn qpc_frequency(&self) -> u64 {
        self.0.qpc_frequency.get()
    }
    fn tsc(&self) -> u64 {
        self.0.tsc.get()
    }
    fn publish(&self, cycles_to_ms: f32, ms_to_cycles: f32) {
        self.0.events.borrow_mut().push(Event::Publish {
            cycles_to_ms,
            ms_to_cycles,
        });
    }
}

pub(crate) fn rel_close(got: f64, want: f64) -> bool {
    if want == 0.0 {
        got == 0.0
    } else {
        ((got - want) / want).abs() <= 1e-6
    }
}

impl Broadcast for HostRef {
    fn post(&self, m: &Message) {
        let h = &self.0;
        h.events.borrow_mut().push(Event::Post {
            msg: *m,
            t: h.now.get(),
        });
    }
}

// ---- helpers -----------------------------------------------------------------------------

pub(crate) fn field<'a>(op: &'a Json, key: &str) -> R<&'a Json> {
    op.get(key).ok_or_else(|| format!("missing field '{key}'"))
}

pub(crate) fn int(op: &Json, key: &str) -> R<i64> {
    field(op, key)?
        .as_i64()
        .ok_or_else(|| format!("field '{key}' is not an integer"))
}

pub(crate) fn opt_int(op: &Json, key: &str) -> R<Option<i64>> {
    match op.get(key) {
        None => Ok(None),
        Some(v) => v
            .as_i64()
            .map(Some)
            .ok_or_else(|| format!("field '{key}' is not an integer")),
    }
}

pub(crate) fn boolean(j: &Json) -> Option<bool> {
    j.as_bool().or_else(|| j.as_i64().map(|v| v != 0))
}

pub(crate) fn check_expect_bool(op: &Json, got: bool) -> R<()> {
    if let Some(e) = op.get("expect") {
        let want = boolean(e).ok_or("'expect' must be a boolean")?;
        if want != got {
            return Err(format!("returned {got}, expected {want}"));
        }
    }
    Ok(())
}

fn level_arg(op: &Json) -> R<u32> {
    Ok(int(op, "level")? as u32)
}

fn hex_decode(s: &str) -> R<Vec<u8>> {
    let clean: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if clean.len() % 2 != 0 {
        return Err("hex string has odd length".into());
    }
    (0..clean.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&clean[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

fn hex_encode(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Encode a record description (README "record object"). Written
/// independently of fc4-core's encoder so the two cross-check each other.
fn encode_record(j: &Json) -> R<Vec<u8>> {
    let known = [
        "version",
        "in_tick",
        "city_established",
        "pause_count",
        "sim_speed",
        "sim_time",
        "frame_budget_ms",
        "level_counters",
        "level_fired",
        "julian",
        "date",
    ];
    for (k, _) in j.as_object().ok_or("record must be an object")? {
        if !known.contains(&k.as_str()) {
            return Err(format!("unknown record field '{k}'"));
        }
    }
    let get = |k: &str, d: i64| -> R<i64> { Ok(opt_int(j, k)?.unwrap_or(d)) };
    let mut out = Vec::new();
    out.extend_from_slice(&(get("version", 4)? as u16).to_le_bytes());
    out.push(u8::from(j.get("in_tick").and_then(boolean).unwrap_or(false)));
    out.push(u8::from(j.get("city_established").and_then(boolean).unwrap_or(false)));
    out.extend_from_slice(&(get("pause_count", 0)? as i32).to_le_bytes());
    out.extend_from_slice(&(get("sim_speed", 3)? as i32).to_le_bytes());
    out.extend_from_slice(&(get("sim_time", 0)? as i32).to_le_bytes());
    out.extend_from_slice(&(get("frame_budget_ms", 15)? as u32).to_le_bytes());
    let counters = level_array(j.get("level_counters"), |v| v.as_i64())?;
    let fired = level_array(j.get("level_fired"), |v| boolean(v).map(i64::from))?;
    for l in 0..LEVEL_COUNT {
        out.extend_from_slice(&(counters[l] as u32).to_le_bytes());
        out.push(fired[l] as u8);
    }
    let jd = if let Some(d) = j.get("date") {
        julian::julian_from_civil(int(d, "year")? as i32, int(d, "month")? as i32, int(d, "day")? as i32) as i64
    } else {
        get("julian", julian::JULIAN_2000_01_01 as i64)?
    };
    out.extend_from_slice(&(jd as u32).to_le_bytes());
    Ok(out)
}

/// An 11-entry per-level array given either as a full array or as an
/// object {"level": value}; missing entries are 0.
fn level_array(j: Option<&Json>, conv: impl Fn(&Json) -> Option<i64>) -> R<[i64; LEVEL_COUNT]> {
    let mut out = [0i64; LEVEL_COUNT];
    match j {
        None => {}
        Some(Json::Array(a)) => {
            if a.len() != LEVEL_COUNT {
                return Err(format!("per-level array must have {LEVEL_COUNT} entries"));
            }
            for (l, v) in a.iter().enumerate() {
                out[l] = conv(v).ok_or("bad per-level value")?;
            }
        }
        Some(Json::Object(kv)) => {
            for (k, v) in kv {
                let l: usize = k.parse().map_err(|_| format!("bad level key '{k}'"))?;
                if l >= LEVEL_COUNT {
                    return Err(format!("level {l} out of range"));
                }
                out[l] = conv(v).ok_or("bad per-level value")?;
            }
        }
        Some(_) => return Err("per-level value must be an array or object".into()),
    }
    Ok(out)
}

fn parse_tuning(j: Option<&Json>) -> R<Tuning> {
    let mut t = Tuning::default();
    if let Some(j) = j {
        if let Some(a) = j.get("ms_per_day") {
            let v = a.as_array().ok_or("ms_per_day must be an array")?;
            t.ms_per_day = Some(
                v.iter()
                    .map(|x| x.as_i64().map(|i| i as u32).ok_or("bad ms_per_day"))
                    .collect::<Result<_, _>>()?,
            );
        }
        if let Some(a) = j.get("animation_dilation") {
            let v = a.as_array().ok_or("animation_dilation must be an array")?;
            t.animation_dilation = Some(
                v.iter()
                    .map(|x| x.as_f64().map(|f| f as f32).ok_or("bad animation_dilation"))
                    .collect::<Result<_, _>>()?,
            );
        }
    }
    Ok(t)
}

fn matches_filter(h: &HostState, e: &Event, filter: Option<&Json>) -> R<bool> {
    let Some(f) = filter else { return Ok(true) };
    for (k, v) in f.as_object().ok_or("filter must be an object")? {
        let ok = match k.as_str() {
            "event" => {
                let want = v.as_str().ok_or("filter.event must be a string")?;
                want == e.kind()
            }
            "agent" => e.agent() == Some(v.as_str().ok_or("filter.agent must be a string")?),
            "type" => e.msg().map(|m| m.msg_type) == Some(parse_type(v)?),
            // Any other event key matches as in `expect_events`.
            _ => event_matches(e, &Json::Object(vec![(k.clone(), v.clone())]))?,
        };
        if !ok {
            return Ok(false);
        }
    }
    let _ = h;
    Ok(true)
}

fn event_matches(e: &Event, want: &Json) -> R<bool> {
    for (k, v) in want.as_object().ok_or("expected event must be an object")? {
        let ok = match k.as_str() {
            "event" => v.as_str() == Some(e.kind()),
            "agent" => e.agent() == v.as_str(),
            "type" => e.msg().map(|m| m.msg_type) == Some(parse_type(v)?),
            "data1" => e.msg().map(|m| m.data1) == v.as_i64(),
            "data2" => e.msg().map(|m| m.data2) == v.as_i64(),
            "data3" => e.msg().map(|m| m.data3) == v.as_i64(),
            "t" => e.t().map(i64::from) == v.as_i64(),
            "void1" | "unknown" => match e {
                Event::BudgetPost { void1, unknown, .. } => {
                    v.as_str() == Some(if k == "void1" { void1.as_str() } else { unknown.as_str() })
                }
                _ => false,
            },
            "series" => match e {
                Event::History { series, .. } | Event::HistoryQueue { series, .. } => {
                    v.as_i64().map(|x| x as u32) == Some(*series)
                }
                _ => false,
            },
            "value" => match e {
                Event::History { value, .. } => v.as_i64() == Some(i64::from(*value)),
                _ => false,
            },
            "length" => match e {
                Event::HistoryQueue { length, .. } => v.as_i64() == Some(i64::from(*length)),
                _ => false,
            },
            "panel" => match e {
                Event::ShowBudgetWindow { panel } => v.as_i64() == Some(i64::from(*panel)),
                _ => false,
            },
            "id" => match e {
                Event::Disaster { id, .. } => v.as_i64().map(|x| x as u32) == Some(*id),
                _ => false,
            },
            "x" | "z" => match e {
                Event::Disaster { x, z, .. } => {
                    let got = f64::from(if k == "x" { *x } else { *z });
                    v.as_f64().is_some_and(|w| rel_close(got, w))
                }
                _ => false,
            },
            "cycles_to_ms" | "ms_to_cycles" => match e {
                Event::Publish {
                    cycles_to_ms,
                    ms_to_cycles,
                } => {
                    let got = if k == "cycles_to_ms" {
                        *cycles_to_ms
                    } else {
                        *ms_to_cycles
                    };
                    let want = v.as_f64().ok_or("publish values must be numbers")?;
                    rel_close(f64::from(got), want)
                }
                _ => false,
            },
            other => return Err(format!("unknown event key '{other}'")),
        };
        if !ok {
            return Ok(false);
        }
    }
    Ok(true)
}

fn filtered(h: &HostState, filter: Option<&Json>) -> R<Vec<Event>> {
    let mut out = Vec::new();
    for e in h.events.borrow().iter() {
        if matches_filter(h, e, filter)? {
            out.push(e.clone());
        }
    }
    Ok(out)
}

fn dump(events: &[Event]) -> String {
    let mut s = String::new();
    for e in events.iter().take(60) {
        s.push_str("\n    ");
        s.push_str(&e.to_json());
    }
    if events.len() > 60 {
        s.push_str(&format!("\n    ... ({} more)", events.len() - 60));
    }
    s
}

fn check_state(snap: &Snapshot, want: &Json) -> R<()> {
    let mut errors = Vec::new();
    let mut cmp_i = |name: &str, got: i64, v: &Json| match v.as_i64().or_else(|| boolean(v).map(i64::from)) {
        Some(w) if w == got => {}
        Some(w) => errors.push(format!("{name}: got {got}, expected {w}")),
        None => errors.push(format!("{name}: bad expected value {v:?}")),
    };
    let mut float_errors = Vec::new();
    let mut per_level = Vec::new();
    for (k, v) in want.as_object().ok_or("state must be an object")? {
        match k.as_str() {
            "initialized" => cmp_i(k, snap.initialized.into(), v),
            "in_tick" => cmp_i(k, snap.in_tick.into(), v),
            "city_established" => cmp_i(k, snap.city_established.into(), v),
            "pause_count" => cmp_i(k, snap.pause_count.into(), v),
            "hidden_pause_count" => cmp_i(k, snap.hidden_pause_count.into(), v),
            "emergency_pause_count" => cmp_i(k, snap.emergency_pause_count.into(), v),
            "sim_speed" => cmp_i(k, snap.sim_speed.into(), v),
            "sim_time" => cmp_i(k, snap.sim_time.into(), v),
            "speed_change_enabled" => cmp_i(k, snap.speed_change_enabled.into(), v),
            "frame_budget_ms" => cmp_i(k, snap.frame_budget_ms.into(), v),
            "gate_level" => cmp_i(k, snap.gate_level.into(), v),
            "ms_per_day" => cmp_i(k, snap.ms_per_day.into(), v),
            "target_fps" => cmp_i(k, snap.target_fps.into(), v),
            "julian" => cmp_i(k, snap.julian.into(), v),
            "idle_cursor" => cmp_i(k, snap.idle_cursor.map_or(-1, |c| c as i64), v),
            "ms_per_day_table" => {
                let got: Vec<i64> = snap.ms_per_day_table.iter().map(|&x| x.into()).collect();
                let w: Option<Vec<i64>> = v.as_array().map(|a| a.iter().filter_map(Json::as_i64).collect());
                if w.as_ref() != Some(&got) {
                    float_errors.push(format!("ms_per_day_table: got {got:?}, expected {v:?}"));
                }
            }
            "animation_dilation" => {
                let w: Vec<f64> = v
                    .as_array()
                    .ok_or("animation_dilation must be an array")?
                    .iter()
                    .filter_map(Json::as_f64)
                    .collect();
                let got: Vec<f64> = snap.animation_dilation.iter().map(|&x| f64::from(x)).collect();
                if w.len() != got.len() || w.iter().zip(&got).any(|(a, b)| (a - b).abs() > 1e-6) {
                    float_errors.push(format!("animation_dilation: got {got:?}, expected {w:?}"));
                }
            }
            "level_counters" | "level_fired" | "agent_counts" | "cursors" => per_level.push((k.clone(), v.clone())),
            other => return Err(format!("unknown state field '{other}'")),
        }
    }
    for (k, v) in per_level {
        let got: [i64; LEVEL_COUNT] = std::array::from_fn(|l| match k.as_str() {
            "level_counters" => snap.level_counters[l].into(),
            "level_fired" => snap.level_fired[l].into(),
            "agent_counts" => snap.agent_counts[l] as i64,
            _ => snap.cursors[l].map_or(-1, |c| c as i64),
        });
        let conv = |x: &Json| {
            x.as_i64()
                .or_else(|| boolean(x).map(i64::from))
                .or(if *x == Json::Null { Some(-1) } else { None })
        };
        match &v {
            Json::Array(a) => {
                let w: Vec<Option<i64>> = a.iter().map(conv).collect();
                if w.len() != LEVEL_COUNT || w.iter().zip(got.iter()).any(|(a, b)| *a != Some(*b)) {
                    errors.push(format!("{k}: got {got:?}, expected {v:?}"));
                }
            }
            Json::Object(kv) => {
                for (lk, lv) in kv {
                    let l: usize = lk.parse().map_err(|_| format!("bad level key '{lk}'"))?;
                    if l >= LEVEL_COUNT || conv(lv) != Some(got[l]) {
                        errors.push(format!(
                            "{k}[{lk}]: got {}, expected {lv:?}",
                            got.get(l).copied().unwrap_or(0)
                        ));
                    }
                }
            }
            _ => return Err(format!("{k} must be an array or object")),
        }
    }
    errors.extend(float_errors);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!("state mismatch: {}", errors.join("; ")))
    }
}

// ---- operations --------------------------------------------------------------------------

fn exec(sim: &Simulator, host: &HostRef, op: &Json) -> R<()> {
    let h = &*host.0;
    let name = field(op, "op")?.as_str().ok_or("'op' must be a string")?;
    match name {
        "init" => {
            let t = parse_tuning(op.get("tuning"))?;
            check_expect_bool(op, sim.init(&t))
        }
        "shutdown" => check_expect_bool(op, sim.shutdown()),
        "repeat" => {
            let n = int(op, "count")?;
            let body = field(op, "steps")?.as_array().ok_or("'steps' must be an array")?;
            for i in 0..n {
                for (j, s) in body.iter().enumerate() {
                    exec(sim, host, s).map_err(|e| format!("repeat iteration {i}, step {j}: {e}"))?;
                }
            }
            Ok(())
        }
        "tick" => {
            let n = opt_int(op, "count")?.unwrap_or(1);
            for _ in 0..n {
                check_expect_bool(op, sim.on_tick())?;
            }
            Ok(())
        }
        "advance" => {
            h.now.set(h.now.get().wrapping_add(int(op, "ms")? as u32));
            Ok(())
        }
        "set_clock" => {
            if let Some(t) = opt_int(op, "now_ms")? {
                h.now.set(t as u32);
            }
            if let Some(s) = opt_int(op, "auto_step_ms")? {
                h.auto_step.set(s as u32);
            }
            Ok(())
        }
        "simulate_time_passage" => {
            sim.simulate_time_passage();
            Ok(())
        }
        "simulate_agents" => {
            sim.simulate_agents();
            Ok(())
        }
        "simulate_idle_time" => {
            sim.simulate_idle_time();
            Ok(())
        }
        "begin_new_interval" => {
            sim.begin_new_interval(level_arg(op)?);
            Ok(())
        }
        "pause" => check_expect_bool(op, sim.pause()),
        "resume" => check_expect_bool(op, sim.resume()),
        "hidden_pause" => check_expect_bool(op, sim.hidden_pause()),
        "hidden_resume" => check_expect_bool(op, sim.hidden_resume()),
        "post_city_init" => check_expect_bool(op, sim.on_post_city_init()),
        "emergency_pause" => check_expect_bool(op, sim.emergency_pause()),
        "emergency_resume" => check_expect_bool(op, sim.emergency_resume()),
        "set_speed" => check_expect_bool(op, sim.set_sim_speed(int(op, "speed")? as i32)),
        "set_sim_time" => check_expect_bool(op, sim.set_sim_time(int(op, "time")? as i32)),
        "set_max_ms_per_tick" => check_expect_bool(op, sim.set_max_milliseconds_per_tick(int(op, "ms")? as u32)),
        // Tooling (the turbo clock; not simulator.md): see fc4_core::Turbo.
        "set_turbo" => {
            let on = boolean(field(op, "on")?).ok_or("'on' must be a boolean")?;
            let t = Turbo {
                on,
                frame_ms: opt_int(op, "frame_ms")?.map_or(TURBO_DEFAULT_FRAME_MS, |v| v as u32),
                until_julian: opt_int(op, "until")?.unwrap_or(0) as i32,
            };
            check_expect_bool(op, sim.set_turbo(t))
        }
        "expect_turbo" => {
            let t = sim.turbo();
            if let Some(v) = op.get("on") {
                let want = boolean(v).ok_or("'on' must be a boolean")?;
                if want != t.on {
                    return Err(format!("turbo on: got {}, expected {want}", t.on));
                }
            }
            if let Some(w) = opt_int(op, "frame_ms")? {
                if w != i64::from(t.frame_ms) {
                    return Err(format!("turbo frame_ms: got {}, expected {w}", t.frame_ms));
                }
            }
            if let Some(w) = opt_int(op, "until")? {
                if w != i64::from(t.until_julian) {
                    return Err(format!("turbo until: got {}, expected {w}", t.until_julian));
                }
            }
            Ok(())
        }
        "take_turbo_auto_off" => {
            let got = sim.take_turbo_auto_off();
            let want = match field(op, "expect")? {
                Json::Null => None,
                v => Some(v.as_i64().ok_or("'expect' must be an integer or null")? as i32),
            };
            if got != want {
                return Err(format!("turbo auto-off: got {got:?}, expected {want:?}"));
            }
            Ok(())
        }
        "set_city_established" => {
            let v = boolean(field(op, "value")?).ok_or("'value' must be a boolean")?;
            check_expect_bool(op, sim.set_city_established(v))
        }
        "add_agent" => {
            let label = field(op, "agent")?.as_str().ok_or("'agent' must be a string")?;
            let handle = h.handle(label);
            let agent_name = op.get("name").and_then(Json::as_str).unwrap_or(label).to_owned();
            let flags = opt_int(op, "flags")?.unwrap_or(3) as u32;
            check_expect_bool(op, sim.add_agent(handle, level_arg(op)?, &agent_name, flags))
        }
        "remove_agent" => {
            let label = field(op, "agent")?.as_str().ok_or("'agent' must be a string")?;
            let handle = h.handle(label);
            let r = match opt_int(op, "level")? {
                Some(l) => sim.remove_agent(handle, l as u32),
                None => sim.remove_agent_all_levels(handle),
            };
            check_expect_bool(op, r)
        }
        "remove_all_agents" => {
            let r = match opt_int(op, "level")? {
                Some(l) => sim.remove_all_agents_at(l as u32),
                None => sim.remove_all_agents(),
            };
            check_expect_bool(op, r)
        }
        "set_agent_enabled" => {
            let n = field(op, "name")?.as_str().ok_or("'name' must be a string")?;
            let v = boolean(field(op, "enabled")?).ok_or("'enabled' must be a boolean")?;
            check_expect_bool(op, sim.set_agent_enabled(n, v))
        }
        "cheat" => {
            let id = parse_cheat_id(field(op, "id")?)?;
            let text = field(op, "text")?.as_str().ok_or("'text' must be a string")?;
            check_expect_bool(op, sim.handle_cheat(id, text))
        }
        "save" => {
            let slot = field(op, "slot")?.as_str().ok_or("'slot' must be a string")?;
            h.slots.borrow_mut().insert(slot.to_owned(), sim.save());
            Ok(())
        }
        "load" => {
            let mut bytes = if let Some(s) = op.get("slot") {
                let s = s.as_str().ok_or("'slot' must be a string")?;
                h.slots
                    .borrow()
                    .get(s)
                    .cloned()
                    .ok_or_else(|| format!("no saved slot '{s}'"))?
            } else if let Some(x) = op.get("hex") {
                hex_decode(x.as_str().ok_or("'hex' must be a string")?)?
            } else {
                encode_record(field(op, "record")?)?
            };
            if let Some(n) = opt_int(op, "truncate")? {
                bytes.truncate(n as usize);
            }
            let got = match sim.load(&bytes) {
                Ok(()) => "ok",
                Err(fc4_core::LoadError::UnsupportedVersion(_)) => "unsupported_version",
                Err(fc4_core::LoadError::Truncated) => "truncated",
            };
            let want = op.get("expect").and_then(Json::as_str).unwrap_or("ok");
            if got != want {
                return Err(format!("load returned {got}, expected {want}"));
            }
            Ok(())
        }
        "expect_save" => {
            let got = sim.save();
            let want = if let Some(s) = op.get("slot") {
                let s = s.as_str().ok_or("'slot' must be a string")?;
                h.slots
                    .borrow()
                    .get(s)
                    .cloned()
                    .ok_or_else(|| format!("no saved slot '{s}'"))?
            } else if let Some(x) = op.get("hex") {
                hex_decode(x.as_str().ok_or("'hex' must be a string")?)?
            } else {
                encode_record(field(op, "record")?)?
            };
            if got != want {
                return Err(format!(
                    "save bytes differ:\n    got      {}\n    expected {}",
                    hex_encode(&got),
                    hex_encode(&want)
                ));
            }
            Ok(())
        }
        "expect_state" => check_state(&sim.snapshot(), field(op, "state")?),
        "set_timer_source" => {
            h.qpc_frequency.set(int(op, "qpc_frequency")? as u64);
            let init = field(op, "initial_ms_per_cycle")?
                .as_f64()
                .ok_or("initial_ms_per_cycle must be a number")?;
            check_expect_bool(
                op,
                sim.set_timer_source(Box::new(TimerRef(host.0.clone())), init as f32),
            )
        }
        "set_timers" => {
            if let Some(q) = opt_int(op, "qpc")? {
                h.qpc.set(q as u64);
            }
            if let Some(c) = opt_int(op, "tsc")? {
                h.tsc.set(c as u64);
            }
            Ok(())
        }
        "advance_timers" => {
            if let Some(q) = opt_int(op, "qpc")? {
                h.qpc.set(h.qpc.get().wrapping_add(q as u64));
            }
            if let Some(c) = opt_int(op, "tsc")? {
                h.tsc.set(h.tsc.get().wrapping_add(c as u64));
            }
            Ok(())
        }
        "calibrate_tick" => {
            sim.calibrate_tick();
            Ok(())
        }
        "expect_calibration" => {
            let cal = sim.calibration();
            if let Some(none) = op.get("none") {
                let want_none = boolean(none).ok_or("'none' must be a boolean")?;
                if want_none != cal.is_none() {
                    return Err(format!(
                        "calibration installed = {}, expected {}",
                        cal.is_some(),
                        !want_none
                    ));
                }
                if want_none {
                    return Ok(());
                }
            }
            let cal = cal.ok_or("no timer source installed")?;
            let pairs = [
                ("cycles_to_ms", cal.cycles_to_ms()),
                ("ms_to_cycles", cal.ms_to_cycles()),
            ];
            for (k, got) in pairs {
                if let Some(v) = op.get(k) {
                    let want = v.as_f64().ok_or("calibration values must be numbers")?;
                    if !rel_close(f64::from(got), want) {
                        return Err(format!("{k}: got {got:e}, expected {want:e}"));
                    }
                }
            }
            if let Some(n) = opt_int(op, "samples")? {
                if cal.samples() as i64 != n {
                    return Err(format!("samples: got {}, expected {n}", cal.samples()));
                }
            }
            Ok(())
        }
        "expect_debug" => {
            // Diagnostics view of the day barrier (spec §5.2 step 8). Does not read
            // the fake clock through auto-step (one read).
            let d = sim.debug_info();
            let flags: [(&str, bool); 11] = [
                ("ready", d.ready),
                ("one_shot_or_emergency_pending", d.one_shot_or_emergency_pending),
                ("hours_complete", d.hours_complete),
                ("day_level_done", d.day_level_done),
                ("tomorrow_starts_week", d.tomorrow_starts_week),
                ("tomorrow_starts_month", d.tomorrow_starts_month),
                ("tomorrow_starts_year", d.tomorrow_starts_year),
                ("blocked_by_week", d.blocked_by_week),
                ("blocked_by_month", d.blocked_by_month),
                ("blocked_by_year", d.blocked_by_year),
                ("day_gate_open", d.day_gate_open),
            ];
            for (k, v) in field(op, "debug")?.as_object().ok_or("'debug' must be an object")? {
                let got = if let Some((_, b)) = flags.iter().find(|(n, _)| n == k) {
                    i64::from(*b)
                } else {
                    match k.as_str() {
                        "interval_elapsed_ms" => i64::from(d.interval_elapsed_ms),
                        "ms_per_day" => i64::from(d.ms_per_day),
                        other => return Err(format!("unknown debug field '{other}'")),
                    }
                };
                let want = v
                    .as_i64()
                    .or_else(|| boolean(v).map(i64::from))
                    .ok_or("bad debug value")?;
                if got != want {
                    return Err(format!("debug {k}: got {got}, expected {want}"));
                }
            }
            Ok(())
        }
        "expect_date" => {
            let d = sim.date_fields();
            expect_date_fields(op, &d)
        }
        "expect_julian" => {
            let (y, m, d) = (
                int(op, "year")? as i32,
                int(op, "month")? as i32,
                int(op, "day")? as i32,
            );
            let jd = julian::julian_from_civil(y, m, d);
            let f = julian::date_fields(jd);
            if (f.year, f.month, f.day_of_month) != (y, m, d) {
                return Err(format!("round trip of {y}-{m}-{d} gave {f:?}"));
            }
            if let Some(leap) = op.get("leap_year") {
                let want = boolean(leap).ok_or("'leap_year' must be a boolean")?;
                if julian::is_leap_year(y) != want {
                    return Err(format!(
                        "is_leap_year({y}) = {}, expected {want}",
                        julian::is_leap_year(y)
                    ));
                }
            }
            if let Some(n) = opt_int(op, "days_in_february")? {
                let got = julian::days_in_month(y, 2);
                if i64::from(got) != n {
                    return Err(format!("days in February {y} = {got}, expected {n}"));
                }
            }
            expect_date_fields(op, &f)
        }
        "expect_agent_enabled" => {
            let n = field(op, "name")?.as_str().ok_or("'name' must be a string")?;
            let want = boolean(field(op, "value")?).ok_or("'value' must be a boolean")?;
            let got = sim.get_agent_enabled(n);
            if got != want {
                return Err(format!("GetAgentEnabled({n}) = {got}, expected {want}"));
            }
            Ok(())
        }
        "expect_agent_names" => {
            let want: Vec<&str> = field(op, "names")?
                .as_array()
                .ok_or("'names' must be an array")?
                .iter()
                .filter_map(Json::as_str)
                .collect();
            let got = sim.enumerate_agent_names();
            if got != want {
                return Err(format!("agent names {got:?}, expected {want:?}"));
            }
            Ok(())
        }
        "expect_dilation" => {
            let want = field(op, "value")?.as_f64().ok_or("'value' must be a number")?;
            let got = f64::from(sim.animation_time_dilation());
            if (got - want).abs() > 1e-6 {
                return Err(format!("dilation {got}, expected {want}"));
            }
            Ok(())
        }
        "expect_paused" => {
            let checks: [(&str, bool); 4] = [
                ("paused", sim.is_paused()),
                ("hidden", sim.is_hidden_paused()),
                ("emergency", sim.is_emergency_paused()),
                ("any", sim.is_any_paused()),
            ];
            for (k, got) in checks {
                if let Some(v) = op.get(k) {
                    let want = boolean(v).ok_or("expect_paused values must be booleans")?;
                    if want != got {
                        return Err(format!("{k}: got {got}, expected {want}"));
                    }
                }
            }
            Ok(())
        }
        "clear_events" => {
            h.events.borrow_mut().clear();
            Ok(())
        }
        "expect_events" => {
            let want = field(op, "events")?.as_array().ok_or("'events' must be an array")?;
            let got = filtered(h, op.get("filter"))?;
            h.events.borrow_mut().clear();
            let mut ok = got.len() == want.len();
            if ok {
                for (e, w) in got.iter().zip(want) {
                    if !event_matches(e, w)? {
                        ok = false;
                        break;
                    }
                }
            }
            if !ok {
                let want_s: Vec<String> = want.iter().map(|w| format!("{w:?}")).collect();
                return Err(format!(
                    "events differ.\n  got ({}):{}\n  expected ({}):\n    {}",
                    got.len(),
                    dump(&got),
                    want.len(),
                    want_s.join("\n    ")
                ));
            }
            Ok(())
        }
        "expect_event_count" => {
            let got = filtered(h, op.get("filter"))?.len() as i64;
            if let Some(n) = opt_int(op, "count")? {
                if got != n {
                    return Err(format!("event count {got}, expected {n}"));
                }
            }
            if let Some(n) = opt_int(op, "min")? {
                if got < n {
                    return Err(format!("event count {got}, expected at least {n}"));
                }
            }
            if let Some(n) = opt_int(op, "max")? {
                if got > n {
                    return Err(format!("event count {got}, expected at most {n}"));
                }
            }
            Ok(())
        }
        "expect_spacing" => {
            let ev = filtered(h, op.get("filter"))?;
            let ts: Vec<i64> = ev.iter().filter_map(Event::t).map(i64::from).collect();
            for w in ts.windows(2) {
                let d = w[1] - w[0];
                if let Some(min) = opt_int(op, "min_ms")? {
                    if d < min {
                        return Err(format!("events {} ms apart (< {min}):{}", d, dump(&ev)));
                    }
                }
                if let Some(max) = opt_int(op, "max_ms")? {
                    if d > max {
                        return Err(format!("events {} ms apart (> {max}):{}", d, dump(&ev)));
                    }
                }
            }
            Ok(())
        }
        "expect_sequence" => {
            let ev = filtered(h, op.get("filter"))?;
            let fname = field(op, "field")?.as_str().ok_or("'field' must be a string")?;
            let start = int(op, "start")?;
            let step = opt_int(op, "step")?.unwrap_or(1);
            for (i, e) in ev.iter().enumerate() {
                let m = e.msg().ok_or("expect_sequence needs message events")?;
                let v = match fname {
                    "data1" => m.data1,
                    "data2" => m.data2,
                    "data3" => m.data3,
                    _ => return Err(format!("bad field '{fname}'")),
                };
                let want = start + step * i as i64;
                if v != want {
                    return Err(format!("event #{i}: {fname} = {v}, expected {want}:{}", dump(&ev)));
                }
            }
            Ok(())
        }
        other => crate::foundations::exec(other, op)
            .or_else(|| crate::budget::exec(sim, host, other, op))
            .or_else(|| crate::demand::exec(host, other, op))
            .or_else(|| crate::ordinance::exec(host, other, op))
            .or_else(|| crate::weather::exec(host, other, op))
            .or_else(|| crate::simgrid::exec(host, other, op))
            .or_else(|| crate::landvalue::exec(host, other, op))
            .or_else(|| crate::grid::exec(host, other, op))
            .or_else(|| crate::flammability::exec(host, other, op))
            .or_else(|| crate::pollution::exec(host, other, op))
            .or_else(|| crate::crime::exec(host, other, op))
            .or_else(|| crate::aura::exec(host, other, op))
            .or_else(|| crate::power::exec(host, other, op))
            .or_else(|| crate::plumbing::exec(host, other, op))
            .or_else(|| crate::protection::exec(host, other, op))
            .or_else(|| crate::flora::exec(host, other, op))
            .or_else(|| crate::development::exec(host, other, op))
            .or_else(|| crate::development_simulator::exec(host, other, op))
            .or_else(|| crate::growth_developer::exec(host, other, op))
            .or_else(|| crate::tract_developer::exec(host, other, op))
            .or_else(|| crate::lot_developer::exec(host, other, op))
            .or_else(|| crate::zones::exec(host, other, op))
            .or_else(|| crate::landfill::exec(host, other, op))
            .or_else(|| crate::neighbors::exec(host, other, op))
            .or_else(|| crate::residential::exec(host, other, op))
            .or_else(|| crate::traffic_network::exec(host, other, op))
            .or_else(|| crate::pathfinder::exec(host, other, op))
            .or_else(|| crate::traffic_simulator::exec(host, other, op))
            .or_else(|| crate::history::exec(host, other, op))
            .or_else(|| crate::clock24::exec(host, other, op))
            .or_else(|| crate::commercial::exec(host, other, op))
            .or_else(|| crate::civic::exec(host, other, op))
            .or_else(|| crate::parks::exec(host, other, op))
            .or_else(|| crate::network_manager::exec(host, other, op))
            .or_else(|| crate::network_rules::exec(host, other, op))
            .or_else(|| crate::network_geometry::exec(host, other, op))
            .or_else(|| crate::city_detail::exec(host, other, op))
            .or_else(|| crate::port_developer::exec(host, other, op))
            .or_else(|| crate::prop_developer::exec(host, other, op))
            .or_else(|| crate::demolition::exec(host, other, op))
            .or_else(|| crate::dispatch::exec(host, other, op))
            .or_else(|| crate::city_situation::exec(host, other, op))
            .or_else(|| crate::audio::exec(host, other, op))
            .or_else(|| crate::automata::exec(host, other, op))
            .or_else(|| crate::disasters::exec(host, other, op))
            .or_else(|| crate::prop_lotconfig::exec(host, other, op))
            .or_else(|| crate::prop_occupant::exec(host, other, op))
            .or_else(|| crate::tutorial::exec(host, other, op))
            .or_else(|| crate::occupant_manager::exec(host, other, op))
            .or_else(|| crate::terrain::exec(host, other, op))
            .or_else(|| crate::advisors::exec(host, other, op))
            .or_else(|| crate::mysims::exec(host, other, op))
            .or_else(|| crate::effects::exec(host, other, op))
            .or_else(|| crate::network_bridge::exec(host, other, op))
            .or_else(|| crate::network_geometry::exec(host, other, op))
            .or_else(|| crate::ui_windows::exec(host, other, op))
            .or_else(|| crate::dates::exec(host, other, op))
            .unwrap_or_else(|| Err(format!("unknown op '{other}'"))),
}}

fn expect_date_fields(op: &Json, d: &julian::DateFields) -> R<()> {
    let pairs = [
        ("julian", d.julian),
        ("year", d.year),
        ("month", d.month),
        ("day", d.day_of_month),
        ("day_of_year", d.day_of_year),
        ("weekday", d.weekday),
    ];
    for (k, got) in pairs {
        if let Some(w) = opt_int(op, k)? {
            if w != i64::from(got) {
                return Err(format!("{k}: got {got}, expected {w} (date {d:?})"));
            }
        }
    }
    Ok(())
}

// ---- vector ------------------------------------------------------------------------------

fn parse_behaviour(j: &Json) -> R<Behaviour> {
    let mut b = Behaviour {
        finish_after: 1,
        ..Default::default()
    };
    for (k, v) in j.as_object().ok_or("agent behaviour must be an object")? {
        match k.as_str() {
            "finish_after" => b.finish_after = v.as_i64().ok_or("finish_after must be an integer")? as u32,
            "cost_ms" => b.cost_ms = v.as_i64().ok_or("cost_ms must be an integer")? as u32,
            "on_call" => {
                for (n, ops) in v.as_object().ok_or("on_call must be an object")? {
                    let n: u32 = n.parse().map_err(|_| format!("bad on_call key '{n}'"))?;
                    b.on_call
                        .insert(n, ops.as_array().ok_or("on_call entries must be arrays")?.to_vec());
                }
            }
            "description" => {}
            other => return Err(format!("unknown agent behaviour key '{other}'")),
        }
    }
    Ok(b)
}

/// Run a vector given as JSON text. Returns the number of steps executed.
pub fn run_vector_text(text: &str) -> R<usize> {
    let v = Json::parse(text).map_err(|e| e.to_string())?;
    let fmt = int(&v, "format")?;
    if fmt != FORMAT_VERSION {
        return Err(format!("unsupported vector format {fmt}"));
    }
    let state = Rc::new(HostState {
        now: Cell::new(0),
        auto_step: Cell::new(0),
        reads: Cell::new(0),
        events: RefCell::new(Vec::new()),
        agents: RefCell::new(HashMap::new()),
        labels: RefCell::new(HashMap::new()),
        names: RefCell::new(HashMap::new()),
        error: RefCell::new(None),
        slots: RefCell::new(HashMap::new()),
        qpc: Cell::new(0),
        qpc_frequency: Cell::new(0),
        tsc: Cell::new(0),
        budget: RefCell::new(None),
        ext: RefCell::new(HashMap::new()),
        grids: RefCell::new(None),
        landvalue: RefCell::new(None),
    });
    if let Some(c) = v.get("clock") {
        state.now.set(opt_int(c, "start_ms")?.unwrap_or(0) as u32);
        state.auto_step.set(opt_int(c, "auto_step_ms")?.unwrap_or(0) as u32);
    }
    let host = HostRef(state.clone());
    if let Some(agents) = v.get("agents") {
        for (label, b) in agents.as_object().ok_or("'agents' must be an object")? {
            let handle = state.handle(label);
            let behaviour = parse_behaviour(b).map_err(|e| format!("agent {label}: {e}"))?;
            state.agents.borrow_mut().insert(
                handle,
                AgentRuntime {
                    behaviour,
                    ..Default::default()
                },
            );
        }
    }
    let sim = Simulator::new(Box::new(host.clone()));
    let steps = field(&v, "steps")?.as_array().ok_or("'steps' must be an array")?;
    for (i, op) in steps.iter().enumerate() {
        state.reads.set(0);
        let label = op.get("op").and_then(Json::as_str).unwrap_or("?");
        let r = exec(&sim, &host, op);
        if let Some(e) = state.error.borrow_mut().take() {
            return Err(format!("step {i} ({label}): {e}"));
        }
        r.map_err(|e| format!("step {i} ({label}): {e}"))?;
    }
    Ok(steps.len())
}

/// Run a vector file.
pub fn run_vector_file(path: &Path) -> R<usize> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    run_vector_text(&text).map_err(|e| format!("{}: {e}", path.display()))
}
