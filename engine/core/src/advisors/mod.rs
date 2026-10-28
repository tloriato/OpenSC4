// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The advisor system (`cSC4AdvisorSystem`, class `0x4A0DD616`), its
//! advisors (`cSC4Advisor` and the seven advisor classes), advices
//! (`cSC4Advice`, `cSC4RewardAdvice`, `cSC4AdviceNeghborDeal`) and advice
//! subjects (`cSC4AdviceSubject`), `docs/specs/advisors.md`. Section
//! references (`§n`) point at that spec.
//!
//! # Structure
//!
//! - `system.rs`: [`AdvisorSystem`], the state of §3.1, `Init` / `Shutdown`
//!   and the script loading of §4.1–§4.3 and §4.6, the ticker and the system
//!   messages (§5.1, §5.2), events (§6.3), the context-advice stack (§6.7),
//!   the popup thresholds (§6.8), the `game` value setters (§7.1), the
//!   ticker variables (§7.2) and the record of §11.1.
//! - `advisor.rs`: [`Advisor`], every advisor class (§1.1): `Init` /
//!   `Shutdown` (§4.4), `Simulate` (§5.3), the day pass and the events
//!   (§6.2–§6.5), the debug messages, the base and subclass messages (§5.4,
//!   §5.5), and the records of §11.2, §11.5 and §11.6.
//! - `variables.rs`: the Lua `game` variables of §7.3–§7.8 and the Safety
//!   disaster messages (§7.6).
//! - `advice.rs`: [`Advice`] (§2.2, §2.3, §3.3, §4.5, §6.1, §6.6, §6.9) and
//!   its record (§11.3).
//! - `subject.rs`: [`AdviceSubject`] (§2.5, §12.4) and its record (§11.4).
//! - `lua.rs`: the `game.*` functions of §8.3 and the globals.
//! - `tests.rs`: a fake host and the §17 tests.
//!
//! Everything outside these classes goes through [`AdvisorHost`]: the Lua
//! state (the `advisors` and `advices` tables, compiled triggers, reward
//! conditions, the `game` table), the city and the simulator date, the
//! message server, the subsystems the variables read ([`Src`], [`ListKind`]),
//! the civic building simulator's conditional-building status, the history
//! warehouse, the ordinances, the 3D view's global effects and save
//! references. Advisors, advices and subjects are core objects shared as
//! `Rc`s; their `serial` (unique per system, from 1) identifies them to the
//! host.
//!
//! MySims (`docs/specs/mysims.md`) are advisors of a class this module does
//! not know: [`AdvisorSystem::register_advisor_class`] adds a class whose
//! behaviour an [`AdvisorExt`] supplies (`crate::mysims`).
//!
//! # Choices (`[choice]`) and literal readings (`[question]`)
//!
//! - **Pointer order** (the advisors' active maps keyed by advice pointer,
//!   the system's event subscriptions ordered by advisor pointer, §2.4,
//!   §3.1): FreeCity4 orders by the object's serial (creation order)
//!   [choice].
//! - **Session IDs** (`GetSessionID`, advice slot 13): the advice's serial
//!   [choice]; the spec gives no rule.
//! - **Re-entrancy**: a trigger or a reward condition runs Lua, which can
//!   call `retire_advice()` or `expire_advice()` on the advice being
//!   evaluated. The active-advice pass (§6.2 step 3) walks a snapshot of the
//!   active map and skips an advice that is no longer in it [choice]; the
//!   event pass (§6.3) handles the events queued when it starts, and events
//!   pushed while it runs wait for the next day pass [question].
//! - **Unknown advice classes**: `CreateInstance` of a class nobody
//!   registered fails, so the entry is not created (§4.4 step 5 then skips
//!   it, and its `Init` is not called) [choice]. Registered: `0x8A09F5F4`,
//!   `0xAA371C32` and `0x6A3848DB`; [`AdvisorSystem::register_advice_class`]
//!   adds others.
//! - **Missing sources**: when the host cannot answer a [`Src`] (a missing
//!   subsystem), the variable is not written [choice] (the spec does not
//!   cover missing subsystems; mysims.md §4.4 uses the same rule).
//! - The remaining choices are documented where they are made.

mod advice;
mod advisor;
mod lua;
pub mod scripted;
mod subject;
mod system;
pub mod variables;

#[cfg(test)]
mod tests;

pub use advice::{entry_matches, Advice, AdviceState};
pub use advisor::{pending, AdviceInfo, Advisor, AdvisorState};
pub use lua::{combine_guids, trend_slope_of, LuaValue};
pub use subject::{AdviceSubject, SubjectState};
pub use system::{AdvisorSystem, SystemState};

pub mod archive;

use std::any::Any;
use std::rc::Rc;

use crate::x87::r24;

// ---- identity (§1.1) -----------------------------------------------------------------------------

/// Class IDs (`GetGZCLSID`, §1.1).
pub mod clsid {
    pub const SYSTEM: u32 = 0x4A0D_D616;
    pub const ADVISOR: u32 = 0xEAC9_37B4;
    /// `cSC4AdvisorBase`: a factory exists, but the object reports
    /// [`ADVISOR`] (§11.6).
    pub const ADVISOR_BASE: u32 = 0xCA09_F543;
    pub const CITY_PLANNING: u32 = 0x2A5F_877D;
    pub const CITY_SITUATIONS: u32 = 0x4BE3_72CD;
    pub const FINANCE: u32 = 0xAA52_92D7;
    pub const HQ: u32 = 0x6A5F_8755;
    pub const SAFETY: u32 = 0xEA19_E825;
    pub const TRAFFIC: u32 = 0x4A3A_D3E1;
    pub const UTILITY: u32 = 0xCA2C_2A2F;
    pub const ADVICE: u32 = 0x8A09_F5F4;
    pub const REWARD_ADVICE: u32 = 0xAA37_1C32;
    pub const NEIGHBOR_DEAL_ADVICE: u32 = 0x6A38_48DB;
    pub const SUBJECT: u32 = 0x6A26_D049;
    /// Other specs' advice classes (§1.1): city situations, MySim, MySim
    /// balloon.
    pub const CITY_SITUATION_ADVICE: u32 = 0x8BE3_753B;
    pub const MYSIM_ADVICE: u32 = 0x6A93_35DE;
    pub const MYSIM_BALLOON_ADVICE: u32 = 0x6B70_EFAD;
    /// MySim advisor classes (mysims.md).
    pub const MYSIM: u32 = 0x4A1D_BBBF;
    pub const MY_STREET_SIM: u32 = 0x0BF6_06A5;
}

/// Interface IDs (§1.1).
pub mod iid {
    pub const UNKNOWN: u32 = 0x0000_0001;
    pub const ADVISOR_SYSTEM: u32 = 0x4A0D_D617;
    pub const ADVISOR: u32 = 0xCA09_F544;
    pub const ADVICE: u32 = 0x8A09_F5F5;
    pub const SUBJECT: u32 = 0x6A26_D050;
    pub const MESSAGE_TARGET2: u32 = 0x4522_94AA;
    pub const MESSAGE_TARGET: u32 = 0xE98B_2F57;
    pub const SERIALIZABLE: u32 = 0xE4FD_A3D4;
    /// The neighbour-deal advice's extra interface.
    pub const NEIGHBOR_DEAL_ADVICE: u32 = 0xEA38_48BA;
    /// `cISC4NeighborDeal` (§6.3).
    pub const NEIGHBOR_DEAL: u32 = 0x0773_FEF1;
}

/// The city component (foundations §3.6) and the city getter (§1.2).
pub const CITY_COMPONENT: u32 = 47;
pub const CITY_GETTER_OFFSET: u32 = 0x144;

/// `QueryInterface` of the system (§1.1).
pub fn system_query_interface(iid: u32) -> bool {
    matches!(
        iid,
        iid::UNKNOWN | iid::ADVISOR_SYSTEM | iid::MESSAGE_TARGET2 | iid::MESSAGE_TARGET | iid::SERIALIZABLE
    )
}

/// The advisor classes (§1.1). `Plain` is `cSC4Advisor`; `Base` is
/// `cSC4AdvisorBase` (Environment, Fluff news); `Ext` is a class an
/// [`AdvisorExt`] implements (MySims).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdvisorKind {
    Plain,
    Base,
    CityPlanning,
    CitySituations,
    Finance,
    Hq,
    Safety,
    Traffic,
    Utility,
    Ext(u32),
}

impl AdvisorKind {
    /// The kind `CreateInstance(class)` builds, for the classes of §1.1.
    pub fn from_class(class: u32) -> Option<AdvisorKind> {
        Some(match class {
            clsid::ADVISOR => AdvisorKind::Plain,
            clsid::ADVISOR_BASE => AdvisorKind::Base,
            clsid::CITY_PLANNING => AdvisorKind::CityPlanning,
            clsid::CITY_SITUATIONS => AdvisorKind::CitySituations,
            clsid::FINANCE => AdvisorKind::Finance,
            clsid::HQ => AdvisorKind::Hq,
            clsid::SAFETY => AdvisorKind::Safety,
            clsid::TRAFFIC => AdvisorKind::Traffic,
            clsid::UTILITY => AdvisorKind::Utility,
            _ => return None,
        })
    }

    /// `GetGZCLSID` of the serializable sub-object: the base class reports
    /// `0xEAC937B4` (§11.6 *[quirk]*).
    pub fn class_id(self) -> u32 {
        match self {
            AdvisorKind::Plain | AdvisorKind::Base => clsid::ADVISOR,
            AdvisorKind::CityPlanning => clsid::CITY_PLANNING,
            AdvisorKind::CitySituations => clsid::CITY_SITUATIONS,
            AdvisorKind::Finance => clsid::FINANCE,
            AdvisorKind::Hq => clsid::HQ,
            AdvisorKind::Safety => clsid::SAFETY,
            AdvisorKind::Traffic => clsid::TRAFFIC,
            AdvisorKind::Utility => clsid::UTILITY,
            AdvisorKind::Ext(c) => c,
        }
    }

    /// `cSC4AdvisorBase` or one of its subclasses (base messages, dirty
    /// byte, update_variables on the monthly hook).
    pub fn is_base_family(self) -> bool {
        !matches!(self, AdvisorKind::Plain | AdvisorKind::Ext(_))
    }

    /// The class the record is loaded as (§11.6): `0xCA09F543` records come
    /// back as plain advisors.
    pub fn for_loaded_class(class: u32) -> Option<AdvisorKind> {
        match class {
            clsid::ADVISOR | clsid::ADVISOR_BASE => Some(AdvisorKind::Plain),
            c => AdvisorKind::from_class(c),
        }
    }
}

/// The advice classes this module implements (§1.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdviceKind {
    /// `cSC4Advice` (and classes registered to behave as it).
    Plain,
    Reward,
    NeighborDeal,
}

// ---- messages (§5, §10) --------------------------------------------------------------------------

pub mod msg {
    pub const POST_CITY_INIT: u32 = 0x26D3_1EC1;
    pub const CITY_ESTABLISHED: u32 = 0x26D3_1EC4;
    pub const TICKER: u32 = 0xCA55_01F2;
    pub const CHEAT: u32 = 0x230E_27AC;
    pub const INSERT_OCCUPANT: u32 = 0x99EF_1142;
    pub const REMOVE_OCCUPANT: u32 = 0x99EF_1143;
    pub const POST_CITY_INIT_COMPLETE: u32 = 0xEA8A_E29A;
    pub const ORDINANCE_UPDATED: u32 = 0xC2C0_9554;
    pub const UPDATE_SELF: u32 = 0xCA8F_3DB6;
    /// The base's subscriptions (§4.4), in order.
    pub const BASE_SUBSCRIPTIONS: [u32; 5] = [
        INSERT_OCCUPANT,
        REMOVE_OCCUPANT,
        POST_CITY_INIT,
        POST_CITY_INIT_COMPLETE,
        ORDINANCE_UPDATED,
    ];
    /// The system's subscriptions (§4.1 step 9), in order.
    pub const SYSTEM_SUBSCRIPTIONS: [u32; 3] = [POST_CITY_INIT, CITY_ESTABLISHED, TICKER];

    // posted (§10)
    pub const ADVICE_ADDED: u32 = 0x8A09_FA64;
    pub const ADVICE_REMOVED: u32 = 0x8A09_FA65;
    pub const ADVICE_CHANGED: u32 = 0x8A09_FA66;
    pub const ADVICE_REMOVED_ALL: u32 = 0x8A09_FA67;
    pub const ADVISOR_CHANGED: u32 = 0x8A09_FA68;
    pub const ADVISOR_REMOVED: u32 = 0x8A09_FA69;
    pub const ADVISOR_ADDED: u32 = 0x8A09_FA6A;
    pub const ADVISOR_REMOVED_ALL: u32 = 0x8A09_FA6B;
    pub const ADVICE_COMMAND: u32 = 0x8A09_FA6C;

    // debug (§5.4)
    pub const DEBUG_SHOW_GUIDS: u32 = 0xEA91_7750;
    pub const DEBUG_FORCE_TRIGGER: u32 = 0x4A91_DD6E;
    pub const DEBUG_PRINT: u32 = 0x4A92_ECEF;
    pub const DEBUG_REFRESH: u32 = 0x8A91_7759;
    /// The advisor's debug subscriptions (§4.4 step 7), in order.
    pub const DEBUG_SUBSCRIPTIONS: [u32; 4] = [DEBUG_SHOW_GUIDS, DEBUG_FORCE_TRIGGER, DEBUG_PRINT, DEBUG_REFRESH];

    // subclasses (§5.5)
    pub const CP_UNHANDLED: [u32; 8] = [
        0xEA39_4B74,
        0xCB3B_E815,
        0xEB3B_E415,
        0x2B3B_E44A,
        0x6B3B_E6D8,
        0x0B3B_E81A,
        0x8A39_4DC4,
        0x4A49_0D33,
    ];
    pub const LOT_DEVELOPER_NOTIFICATION: u32 = 0x4C04_DE69;
    pub const DEAL_STATUS: u32 = 0xEA41_8D8F;
    pub const POWER_STRIKE_BEGIN: u32 = 0xCA35_8F85;
    pub const POWER_STRIKE_END: u32 = 0xCA35_8F9C;
    pub const HEALTH_STRIKE: u32 = 0x077A_CCD2;
    pub const SCHOOL_STRIKE: u32 = 0x077A_CCD0;
    pub const JAILBREAK_BEGIN: u32 = 0x2A61_4891;
    pub const JAILBREAK_END: u32 = 0x2A61_4892;
    pub const FIRE_STRIKE_BEGIN: u32 = 0x68F5_4CE1;
    pub const FIRE_STRIKE_END: u32 = 0x68F5_4CE2;
    pub const POLICE_STRIKE_BEGIN: u32 = 0xA8DA_F511;
    pub const POLICE_STRIKE_END: u32 = 0xA8DA_F512;
    pub const TRANSIT_STRIKE_BEGIN: u32 = 0x6924_7DC4;
    pub const TRANSIT_STRIKE_END: u32 = 0x6924_7DC5;
    pub const NUCLEAR_EXPLOSION: u32 = 0xAA55_2F77;
    pub const ZOO_ESCAPE: u32 = 0xAA66_7CE2;
    pub const PIPE_BURST: u32 = 0xEA1F_0DCD;
    pub const ROUTE_BLOCKED: u32 = 0xCA8D_CDB4;
    pub const DISASTER_STARTED: u32 = 0xCA1B_1E64;
    pub const DISASTER_ENDED: u32 = 0xAA1B_2002;
    /// Safety's own subscriptions (§5.5, §7.6), in the order the spec lists
    /// them [choice].
    pub const SAFETY_SUBSCRIPTIONS: [u32; 16] = [
        HEALTH_STRIKE,
        SCHOOL_STRIKE,
        JAILBREAK_BEGIN,
        JAILBREAK_END,
        FIRE_STRIKE_BEGIN,
        FIRE_STRIKE_END,
        POLICE_STRIKE_BEGIN,
        POLICE_STRIKE_END,
        TRANSIT_STRIKE_BEGIN,
        TRANSIT_STRIKE_END,
        NUCLEAR_EXPLOSION,
        ZOO_ESCAPE,
        PIPE_BURST,
        ROUTE_BLOCKED,
        DISASTER_STARTED,
        DISASTER_ENDED,
    ];
    pub const TRAFFIC_SUBSCRIPTIONS: [u32; 1] = [LOT_DEVELOPER_NOTIFICATION];
    pub const UTILITY_SUBSCRIPTIONS: [u32; 3] = [DEAL_STATUS, POWER_STRIKE_BEGIN, POWER_STRIKE_END];
}

/// Cheat codes (§4.1 step 7, §5.2).
pub mod cheat {
    pub const DOLLY_LLAMA: u32 = 0xAAB7_F42A;
    pub const ADV: u32 = 0xA919_D25E;
}

/// Advisor events pushed by this module (§7.9).
pub mod event {
    pub const NEW_CITY: u32 = 0xAA31_A210;
    pub const POWER_STRIKE_BEGIN: u32 = 0xEA31_A30A;
    pub const POWER_STRIKE_END: u32 = 0x2A31_A30F;
    /// Utility deal events for `0xEA418D8F` data1 3, 4, 5, 6 (§5.5).
    pub const DEAL_EVENTS: [u32; 4] = [0x3A38_3059, 0x0A38_3053, 0xAA38_3052, 0x4A38_3049];
    pub const HEALTH_STRIKE_BEGIN: u32 = 0xAA60_FACA;
    pub const HEALTH_STRIKE_END: u32 = 0x2A60_FACF;
    pub const SCHOOL_STRIKE_BEGIN: u32 = 0xEA60_FABF;
    pub const SCHOOL_STRIKE_END: u32 = 0x2A60_FAC7;
    pub const JAILBREAK_BEGIN: u32 = 0xCA61_6650;
    pub const JAILBREAK_END: u32 = 0x6A61_665A;
    pub const FIRE_STRIKE_BEGIN: u32 = 0x0A31_A314;
    pub const FIRE_STRIKE_END: u32 = 0x6A31_A319;
    pub const POLICE_STRIKE_BEGIN: u32 = 0x6A31_A2FE;
    pub const POLICE_STRIKE_END: u32 = 0x8A31_A303;
    pub const ZOO_ESCAPE: u32 = 0x2A66_7FC3;
    pub const PIPE_BURST: u32 = 0x4A31_CF15;
    pub const ROUTE_BLOCKED_0: u32 = 0xCA3F_CEFA;
    pub const ROUTE_BLOCKED_1: u32 = 0xCA3F_CEF4;
    pub const USER_DISASTER_BASE: u32 = 0xCC32_98DC;
    pub const DISASTER_BASE: u32 = 0xCA1B_1E64;
    pub const DISASTER_NOT_USER: u32 = 0xCA3C_0412;
    pub const DISASTER_ENDED: u32 = 0xAA31_A2F7;
    /// MySim building added (`event_building_distance`, §8.3).
    pub const MYSIM_BUILDING_ADD: u32 = 0xCA4E_8986;
}

/// Disaster types (§7.6).
pub mod disaster {
    pub const FIRE: u32 = 0x621C_DA33;
    pub const RIOT: u32 = 0xCA38_0B06;
    pub const PIPE_BURST: u32 = 0x4A13_32E8;
}

/// Advice flags (§2.3).
pub mod flags {
    pub const NEWS_ONLY: u32 = 0x1;
    pub const ACTIVE: u32 = 0x2;
    pub const ONCE: u32 = 0x4;
    pub const EXPIRE_REQUEST: u32 = 0x8;
    pub const TEXT_DIRTY: u32 = 0x10;
    pub const EVENT: u32 = 0x40;
    pub const NO_TEXT: u32 = 0x80;
    pub const PERSIST: u32 = 0x100;
    pub const DEBUG_GUID: u32 = 0x200;
    pub const FORCED_TRIGGER: u32 = 0x400;
    pub const CLASS_MISMATCH: u32 = 0x800;
    pub const REWARD: u32 = 0x1000;
    pub const CITY_SITUATION: u32 = 0x2000;
    pub const POPUP_QUEUED: u32 = 0x4000;
    pub const NO_TIMEOUT: u32 = 0x8000;
}

/// Occupant types and groups (§5.4, §5.5).
pub const OCCUPANT_TYPE_BUILDING: u32 = 0x2781_28A0;
pub const OCCUPANT_TYPE_NETWORK: u32 = 0xC772_BF98;
/// bool property: conditional building (§5.4).
pub const PROP_CONDITIONAL_BUILDING: u32 = 0xEA32_09F8;
pub const GROUP_PARK: u32 = 0x1006;
pub const GROUP_LANDMARK: u32 = 0x150A;

/// `nextDay` of a `once` advice after deactivation (§6.4).
pub const NEVER: i32 = 0x7FFF_FFFF;
/// The compiled trigger reference of an advice without one (§3.3).
pub const NO_TRIGGER: i32 = -2;
/// The paused update period (§5.1 step 3): strictly more than 500 ms.
pub const IDLE_PERIOD_MS: u32 = 500;
/// The monthly step's day of the month (§5.1 step 2.2).
pub const MONTHLY_DAY: i32 = 28;
/// The default popup threshold (§6.8).
pub const DEFAULT_POPUP_THRESHOLD: i32 = 100;
/// `tuning_constants.ADVICE_POPUP_PRIORITIES_BY_SKILL_LEVEL` keys, by
/// difficulty (§6.8).
pub const POPUP_KEYS: [&str; 3] = ["low", "medium", "high"];
/// The trigger text of an advice without one (§4.5 step 3).
pub const DEFAULT_TRIGGER: &[u8] = b"0";
/// The debug force skips advices whose trigger text is this (§5.4).
pub const FORCE_SKIP_TEXT: &[u8] = b"=(0)";
/// The neighbour-deal advice's record version (§11.3).
pub const ND_RECORD_VERSION: u16 = 2;
pub const SYSTEM_RECORD_VERSION: u16 = 1;
pub const ADVISOR_RECORD_VERSION: u16 = 3;
pub const ADVICE_RECORD_VERSION: u16 = 4;
pub const SUBJECT_RECORD_VERSION: u16 = 4;
pub const CITY_PLANNING_RECORD_VERSION: u16 = 3;

/// The batch size of the day pass (§6.2 step 1, float32 per foundations
/// §18): `trunc(f32(f32(n × 0.033333335f) + 1.0f))`.
pub fn batch_size(n: usize) -> usize {
    let a = (n as f32) * 0.033_333_335_f32;
    (a + 1.0f32) as usize
}

/// "×100" (§7.1): `v × 100.0f` rounded once to a 24-bit significand (the
/// x87 keeps its wide exponent, so `FLT_MAX × 100` stays finite, §17 test
/// 20), then stored as a double.
pub fn pct100(v: f32) -> f64 {
    r24_wide(f64::from(v) * 100.0)
}

/// One rounding to a 24-bit significand (nearest even) with the x87's wide
/// exponent: no overflow to infinity above `FLT_MAX` (§17 test 20).
pub fn r24_wide(x: f64) -> f64 {
    if !x.is_finite() || x == 0.0 {
        return x;
    }
    let exp = ((x.to_bits() >> 52) & 0x7FF) as i32 - 1023;
    if !(-126..=1000).contains(&exp) {
        return r24(x);
    }
    let scale = 2f64.powi(23 - exp);
    // |y| < 2^24: adding and removing 2^52 rounds to an integer, ties to
    // even.
    let y = x * scale;
    let big = 4_503_599_627_370_496.0f64.copysign(y);
    ((y + big) - big) / scale
}

// ---- script data (§2.1, §2.2) --------------------------------------------------------------------

/// One entry of the Lua `advisors` table (§2.1).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AdvisorEntry {
    pub class_id: u32,
    pub id: i32,
    /// The whole `caption`; `Init` keeps the text before the first `|`.
    pub caption: Vec<u8>,
    pub name: Vec<u8>,
    pub advice_type: i32,
}

/// One entry of the Lua `advices` table (§2.2). Missing numeric fields are
/// 0, a missing `trigger` is `None`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AdviceEntry {
    pub guid: u32,
    pub class_id: u32,
    /// `type`: the advisor's `advice_type`.
    pub advice_type: i32,
    pub priority: i32,
    pub mood: i32,
    pub frequency: i32,
    pub timeout: i32,
    pub trigger: Option<Vec<u8>>,
    pub once: bool,
    pub news_only: bool,
    /// `event`: 0 = not an event advice.
    pub event: u32,
    pub command: u32,
    pub persist: bool,
    pub no_timeout: bool,
    pub effects: Vec<u32>,
    /// `nd_type` (neighbour-deal advices).
    pub nd_type: i32,
}

/// The value a protected Lua call returned (§6.1, §6.6).
#[derive(Clone, Debug, PartialEq)]
pub enum LuaResult {
    Bool(bool),
    Number(f64),
    Str(Vec<u8>),
    /// Any other type (nil, table, userdata, …).
    Other,
    /// The call raised a Lua error.
    Error,
}

/// `cISC4CivicBuildingSimulator::ConditionalBuildingStatus` (civic-buildings
/// §2), as the reward advices read and write it (§6.6).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConditionalStatus {
    pub status: u8,
    pub building_count: u16,
    pub tooltip: Vec<u8>,
}

/// The status bits (§6.6).
pub mod status_bits {
    pub const HIDDEN: u8 = 1;
    pub const AVAILABLE: u8 = 2;
    pub const ADVISOR_EVENT: u8 = 4;
}

/// What a new subject is filled with (rewards, §6.6).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SubjectInit {
    pub location: Option<(f32, f32)>,
    pub object: u64,
    pub data1: u32,
    pub data2: u32,
}

// ---- messages crossing the host --------------------------------------------------------------------

/// A message as `DoMessage` sees it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AdvisorMessage {
    pub msg_type: u32,
    pub data1: u32,
    pub data2: u32,
    pub data3: u32,
    pub data4: u32,
    /// `GetVoid1()`: the occupant of an insert / remove message; the city
    /// of `PostCityInit`.
    pub void1: Option<u64>,
    /// `GetIGZUnknown()`: the lot of a lot-developer notification, the deal
    /// of a deal status report.
    pub unknown: Option<u64>,
}

impl AdvisorMessage {
    pub fn of_type(msg_type: u32) -> AdvisorMessage {
        AdvisorMessage {
            msg_type,
            ..AdvisorMessage::default()
        }
    }
}

/// The IGZUnknown of an outgoing message.
#[derive(Clone)]
pub enum MsgObj {
    None,
    Advice(Rc<Advice>),
    Advisor(Rc<Advisor>),
    /// A host object (a subject's object, `event_feedback`).
    Host(u64),
}

impl std::fmt::Debug for MsgObj {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MsgObj::None => write!(f, "None"),
            MsgObj::Advice(a) => write!(f, "Advice({})", a.serial()),
            MsgObj::Advisor(a) => write!(f, "Advisor({})", a.serial()),
            MsgObj::Host(t) => write!(f, "Host({t:#x})"),
        }
    }
}

/// An outgoing message (§10).
#[derive(Clone, Debug)]
pub struct OutMessage {
    pub msg_type: u32,
    pub data1: u32,
    pub data2: u32,
    pub unknown: MsgObj,
}

impl OutMessage {
    pub fn new(msg_type: u32, data1: u32, unknown: MsgObj) -> OutMessage {
        OutMessage {
            msg_type,
            data1,
            data2: 0,
            unknown,
        }
    }
}

/// Who subscribes (§4.1, §4.4).
#[derive(Clone)]
pub enum Target {
    System,
    Advisor(Rc<Advisor>),
}

impl std::fmt::Debug for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Target::System => write!(f, "System"),
            Target::Advisor(a) => write!(f, "Advisor({})", a.serial()),
        }
    }
}

/// The simulator's date (§5.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SimDate {
    /// The day number (`GetSimDate` as a Julian day).
    pub day_number: i32,
    pub year: i32,
    pub month: i32,
    pub day: i32,
}

impl SimDate {
    /// The fields of a Julian day number.
    pub fn from_day(day_number: i32) -> SimDate {
        let f = crate::julian::date_fields(day_number);
        SimDate {
            day_number,
            year: f.year,
            month: f.month,
            day: f.day_of_month,
        }
    }
}

// ---- variable sources (§7) -----------------------------------------------------------------------

/// The subsystem reads of the variables (§7.3–§7.8). Codes are part of the
/// C ABI (fc4.h `FC4_ADV_SRC_*`). Arguments: `a`, `b` and `object` as
/// documented per kind.
macro_rules! src_kinds {
    ($($(#[$m:meta])* $name:ident = $code:expr,)*) => {
        #[repr(u32)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum SrcKind {
            $($(#[$m])* $name = $code,)*
        }

        impl SrcKind {
            /// Every kind, in code order.
            pub const ALL: &'static [SrcKind] = &[$(SrcKind::$name,)*];

            /// The kind's name (the variant name), for logs and vectors.
            pub fn name(self) -> &'static str {
                match self {
                    $(SrcKind::$name => stringify!($name),)*
                }
            }
        }
    };
}

src_kinds! {
    // CityPlanning (§7.3)
    /// Aura `+0x20` `GetMayorRating`: `i` (int8).
    MayorRating = 1,
    /// Aura `+0x28` `GetMinMayorRating(&p)`: `i`, cell.
    MinMayorRating = 2,
    /// Aura `+0x24` `GetMaxMayorRating(&p)`: `i`, cell.
    MaxMayorRating = 3,
    /// Demand `GetDemand(a, b)` → `QuerySupplyValue`: `f`.
    DemandSupply = 4,
    /// `GetDemand(a, b)` → `QueryDemandValue`: `f`.
    DemandValue = 5,
    /// `GetDemand(a, b)` → `QueryActiveDemandValue`: `f`.
    DemandActive = 6,
    /// `GetDemand(a, b)` → `GetDemandCap`: `f` (a percentage).
    DemandCap = 7,
    /// Demand simulator `+0x24` `GetJobsBySensus(a)`: `i` (u32).
    JobsByCensus = 8,
    /// Demand simulator `+0x28` `GetNeutralTaxRate`: `f`.
    NeutralTaxRate = 9,
    /// ZoneManager `+0x18` zone count of zone type `a`: `i`.
    ZoneCount = 10,
    /// Neighbors `+0x44` `GetConnectionsOfType(a)` list length: `i`.
    ConnectionsOfType = 11,
    /// Neighbors city set size: `a` 0 = connected `+0x54`, 1 = adjacent
    /// `+0x50`, 2 = connected indirectly `+0x58`: `i`.
    NeighborCities = 12,
    /// Seaport developer `+0x3C` `CityHasSea`: `i` 0/1.
    CityHasSea = 13,
    /// RegionalCity `+0x24` `GetPopulation(a)`: `i` (int32).
    RegionPopulation = 14,
    /// RegionalCity `+0x44` `GetExtrapolatedPopulation(a)`: `i` (int32).
    ExtrapolatedPopulation = 15,
    /// City `+0x164` `SizeX`: `f`.
    CitySizeX = 16,
    /// City `+0x168` `SizeZ`: `f`.
    CitySizeZ = 17,
    /// Terrain `+0xD8`: `i`.
    WaterTileCount = 18,
    /// Budget `+0x70` `GetDepartmentBudget(a)` → `+0x28` funding: `f` (a
    /// fraction).
    DepartmentFunding = 19,
    // Finance (§7.4)
    TotalFunds = 20,
    IncomeMonthly = 21,
    ExpenseMonthly = 22,
    /// Budget `+0x90` `GetTaxRate(a)`: `f`.
    TaxRate = 23,
    TaxIncome = 24,
    Borrowed = 25,
    BorrowingLimit = 26,
    BondPaymentsMonthly = 27,
    /// Neighbors `+0x80`: `i` (int64).
    DealIncome = 28,
    /// Neighbors `+0x84`.
    DealExpense = 29,
    // HQ (§7.5)
    /// Residential `+0x88` `GetGlobalLE`: `i` (uint8).
    GlobalLifeExpectancy = 30,
    /// Residential `+0x84`: `f`.
    GlobalHealth = 31,
    /// Residential `+0x80`: `f`.
    GlobalEducation = 32,
    /// Residential `+0xB0` `GetHQMinAndMaxCellCoords`: the minimum, `f` and
    /// the tract centre's cell.
    HqMin = 33,
    HqMax = 34,
    /// Residential `+0xAC` `GetEQMinAndMaxCellCoords`.
    EqMin = 35,
    EqMax = 36,
    /// Residential `+0x3C` totals over the health group `a` (0 = all
    /// health groups): `n` = served, `i` = population, `f` = Σgrade, `m` =
    /// count.
    HealthTotals = 37,
    /// Residential `+0x38` totals over the school group `a` (0 = all school
    /// groups), as [`SrcKind::HealthTotals`].
    SchoolTotals = 38,
    // Safety (§7.6)
    /// Fire `+0x30` `GetChanceOfStrike`: `f`.
    FireStrikeChance = 40,
    /// Residential `+0x40`: `i`.
    Population = 41,
    /// Flammability `+0x1C` `GetFlammabilityHotspot(&v, &x, &z)`: `i`, cell.
    FlammabilityHotspot = 42,
    /// Crime `+0x18` `GetAverageCrime`: `i` (the low byte is used).
    AverageCrime = 43,
    /// Crime `+0x20` `GetCrimeHotspot`: `i`, cell; no answer = no hotspot.
    CrimeHotspot = 44,
    /// Police `+0x78`.
    JailPopulation = 45,
    /// Police `+0x7C`.
    JailedCount = 46,
    /// Crime `+0x1C` `GetMonthlyCrimeCount`: `i` (u32).
    MonthlyCrimeCount = 47,
    /// Police `+0x88`.
    ArrestCount = 48,
    /// Police `+0x2C`: `f`.
    PoliceStrikeChance = 49,
    /// Residential `+0x28`: `f`.
    SchoolStrikeChance = 50,
    /// Residential `+0x2C`: `f`.
    HealthStrikeChance = 51,
    /// Fire `+0x5C`: `i` (u32).
    FireStationCount = 52,
    /// Police `+0x60`.
    PoliceStationCount = 53,
    /// Police `+0x8C`.
    JailCount = 54,
    /// Flammability `+0x20`: `i` (uint8).
    AverageFlammability = 55,
    /// Fire `+0x4C`: `f`.
    FireCoverage = 56,
    /// Fire `+0x50`: `f` (×100).
    FireCoveragePercent = 57,
    /// Police `+0x48`: `f`.
    PoliceCoverage = 58,
    /// Police `+0x4C`: `f` (×100).
    PoliceCoveragePercent = 59,
    /// Police `+0x74`: `i`.
    JailCapacity = 60,
    /// The disaster layer's statistics: damaged buildings, `i` (u32).
    DisasterDamageBuildings = 61,
    /// Damage cost, `i` (int64).
    DisasterDamageCost = 62,
    /// Police / fire dispatch effectiveness, `f` (×100).
    DisasterPoliceEffectiveness = 63,
    DisasterFireEffectiveness = 64,
    /// The fire cause (`+0x38`) of the disaster `object` (the message's
    /// IGZUnknown): `i`.
    DisasterFireCause = 65,
    // Traffic (§7.7). `a` = network: 0 road, 1 rail, 2 street, 3 avenue,
    // 4 subway, 5 elevated, 6 monorail, 7 bridge; station kind: 0 bus,
    // 1 train, 2 subway, 3 elevated, 4 monorail.
    /// `GetTrafficStats` average congestion of network `a`: `f`.
    CongestionAverage = 70,
    /// Congested tile count of network `a`: `i` (u32).
    CongestionTiles = 71,
    /// Worst congestion of network `a`: `f`, cell.
    CongestionWorst = 72,
    /// Average utilisation of station kind `a`: `f`.
    StationUtilization = 73,
    /// Lowest utilisation of station kind `a`: `f`, cell.
    StationUtilizationLow = 74,
    /// `a` 0 = road funding, 1 = mass transit funding: `f` (a fraction).
    TransportFunding = 75,
    /// `ga_freight_trip_length`: `i` (u8).
    FreightTripLength = 76,
    /// Port developer `+0x14` `GetAvgEfficiency`, `a` 0 = airport,
    /// 1 = seaport: `f`.
    PortEfficiency = 77,
    /// Traffic `+0x28` congestion map `GetCellValue(a, b)`: `i`.
    CongestionAt = 78,
    /// Port `+0x24` `GetNumPorts(sizeMask, stageMask)`: `a` 0 = airport,
    /// 1 = seaport; `b` = sizeMask | stageMask << 8: `i`.
    NumPorts = 79,
    /// Seaport `+0x34`: `i` (u32).
    SeaportVolume = 80,
    /// Tile count of network `a` (0 road, 1 rail, 2 subway, 3 highway,
    /// 4 street, 5 avenue, 6 elevated rail, 7 monorail, 8 dirt road,
    /// 9 ground highway): `i`.
    NetworkTiles = 81,
    PotholeCount = 82,
    FreightDepotCount = 83,
    RapidTransitTransitions = 84,
    /// The transit strike state (`g_transit_strike`): `i` 0/1.
    TransitStrike = 85,
    /// The station count of station kind `a` (names inferred).
    StationCount = 86,
    // Utility (§7.8)
    /// Power `+0x30` `GetMonthlyProduction`: `i`.
    PowerProduction = 90,
    /// Power `+0x44`.
    PowerConsumed = 91,
    /// Power `+0x6C`.
    OverworkedPlants = 92,
    /// Power `+0x54`.
    UnpoweredBuildings = 93,
    /// Power `+0x9C`: `i` 0/1.
    PowerStrike = 94,
    /// Power `+0xA0`: `f`.
    PowerStrikeChance = 95,
    /// RegionalCity `+0xD8` `GetExported(a)`: `f` (double).
    RegionExported = 96,
    /// RegionalCity `+0xE0` `GetImported(a)`: `f` (double).
    RegionImported = 97,
    /// Plumbing `+0x38`: `i` (the low 32 bits are used).
    WaterProduction = 98,
    /// Plumbing `+0x3C`.
    WaterConsumed = 99,
    /// Pollution `+0x7C`: `i`.
    IncineratorCapacity = 100,
    /// Pollution `+0x80`.
    WasteToEnergyCapacity = 101,
    /// Pollution `+0x84`: `f`.
    RecyclingEffect = 102,
    /// Pollution `+0x28`, `+0x2C`, `+0x30`: `i`.
    AirPollution = 103,
    WaterPollution = 104,
    GarbagePollution = 105,
    /// Pollution `+0x60`.
    UncollectedGarbage = 106,
    /// LandfillDeveloper `+0x28` `GetLandfillUsage(&u, &cap)`: `i` = u,
    /// `n` = cap.
    LandfillUsage = 107,
    /// Pollution `+0x8C`, `+0x98`, `+0xA0`, `+0xA4`: `i` (int64).
    GarbageProduced = 108,
    GarbageRecycled = 109,
    GarbageToEnergy = 110,
    GarbageToLandfill = 111,
    /// Pollution `+0x44` / `+0x48` / `+0x4C` worst tract: `i`, cell.
    WorstAirPollution = 112,
    WorstWaterPollution = 113,
    WorstUncollectedGarbage = 114,
    /// Power `+0x60`.
    PowerPlantCount = 115,
    /// Power `+0x5C` plant count of type `a`.
    PowerPlantsOfType = 116,
    /// Power `+0x70`.
    AgedPowerPlants = 117,
    /// Power `+0x68`.
    PowerPoleCount = 118,
    /// Power `+0x78` `FindPowerPlants` (funding preset 1.0, age preset 0):
    /// `f` = lowest funding, `object` = its plant (0 = none), `i` = highest
    /// age, `object2` = its plant.
    PowerPlantExtremes = 119,
    /// Plumbing `+0x30`.
    WaterSourceCount = 120,
    /// Plumbing `+0x48`, `+0x4C`, `+0x50`.
    WaterPipeCount = 121,
    DistressedPipes = 122,
    BurstPipes = 123,
    /// Plumbing `+0x54` / `+0x58`.
    WateredBuildings = 124,
    UnwateredBuildings = 125,
    /// Plumbing `+0x34`.
    PumpShutdowns = 126,
    /// Plumbing `+0x6C`, as [`SrcKind::PowerPlantExtremes`].
    WaterBuildingExtremes = 127,
    /// Pollution `+0xA8`, `+0xAC`, `+0xB4`, `+0xB0`.
    IncineratorCount = 128,
    WasteToEnergyCount = 129,
    WaterTreatmentCount = 130,
    RecyclingCenterCount = 131,
    /// Neighbors `+0x78` `FindDeals(a)` list length.
    DealCount = 132,
    /// Power `+0xC0` funding: `f` (a fraction).
    PowerFunding = 133,
}

impl SrcKind {
    pub fn code(self) -> u32 {
        self as u32
    }

    /// The kind of a C ABI code.
    pub fn from_code(code: u32) -> Option<SrcKind> {
        SrcKind::ALL.iter().copied().find(|k| k.code() == code)
    }

    /// The kind of a name.
    pub fn from_name(name: &str) -> Option<SrcKind> {
        SrcKind::ALL.iter().copied().find(|k| k.name() == name)
    }
}

/// One read of a subsystem value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Src {
    pub kind: SrcKind,
    pub a: u32,
    pub b: u32,
    pub object: u64,
}

impl Src {
    pub fn new(kind: SrcKind) -> Src {
        Src {
            kind,
            a: 0,
            b: 0,
            object: 0,
        }
    }
    pub fn a(kind: SrcKind, a: u32) -> Src {
        Src { a, ..Src::new(kind) }
    }
    pub fn ab(kind: SrcKind, a: u32, b: u32) -> Src {
        Src { a, b, ..Src::new(kind) }
    }
}

/// A subsystem value: the fields a [`SrcKind`] documents.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Val {
    pub i: i64,
    pub f: f64,
    /// A cell (or a position in cells) for a subject.
    pub cell: Option<(f32, f32)>,
    pub object: u64,
    pub object2: u64,
    pub n: u64,
    pub m: u64,
}

impl Val {
    pub fn int(i: i64) -> Val {
        Val { i, ..Val::default() }
    }
    pub fn float(f: f64) -> Val {
        Val { f, ..Val::default() }
    }
    /// The float value as the callee's `float` (the host may hand a double).
    pub fn f32(&self) -> f32 {
        self.f as f32
    }
}

/// The list reads of the variables (§7.5–§7.8). Codes are part of the C
/// ABI.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ListKind {
    /// Residential `+0xC4` `GetHospitalBuildings(list, groups)`: items with
    /// `object`, `f0` = funding fraction (health purpose), `f1` = grade.
    Hospitals = 1,
    /// Residential `+0xBC` `GetSchoolBuildings(list, groups)`: `f0` =
    /// funding fraction (school purpose), `f1` = grade.
    Schools = 2,
    /// Fire `+0x58` stations: `object`, `f0` = funding (`+0x20`), `flags`
    /// bit 0 = the lot exists, bit 1 = the lot has road access.
    FireStations = 3,
    /// Police `+0x54` stations: `f0` = funding (`+0x1C`), flags as fire.
    PoliceStations = 4,
    /// Port `+0x20` `GetAllPortLots`, `a` 0 = airport, 1 = seaport: the lot
    /// (`object`), its cell (`x`, `z`), flags bit 1 = road access.
    PortLots = 5,
    /// Neighbors connections: `a` = connection type (`+0x44`), or 0 for
    /// `GetAllConnections` (`+0x40`): `kind` = type, `x`, `z` = the inner
    /// cell (`+0x1C`).
    Connections = 6,
    /// Neighbors `+0x64` `ProposeDeals(list, false)`: `kind` = the deal
    /// type of each proposal.
    DealProposals = 7,
}

impl ListKind {
    pub fn from_code(code: u32) -> Option<ListKind> {
        use ListKind::*;
        [
            Hospitals,
            Schools,
            FireStations,
            PoliceStations,
            PortLots,
            Connections,
            DealProposals,
        ]
        .into_iter()
        .find(|k| *k as u32 == code)
    }
}

/// One list read.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ListSrc {
    pub kind: Option<ListKind>,
    pub a: u32,
    /// The occupant groups of a building list.
    pub groups: Vec<u32>,
}

/// One list item.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Item {
    pub object: u64,
    pub f0: f32,
    pub f1: f32,
    pub x: f32,
    pub z: f32,
    pub flags: u32,
    pub kind: u32,
}

/// Item flags.
pub const ITEM_LOT_EXISTS: u32 = 1;
pub const ITEM_ROAD_ACCESS: u32 = 2;

// ---- the host -------------------------------------------------------------------------------------

/// Everything the advisor system reads from, or does to, the rest of the
/// game. Re-entrancy: Lua calls (`call_trigger`, `call_condition`) may call
/// back into the system's Lua functions; every other callback must not
/// call back into the advisor objects.
pub trait AdvisorHost {
    // ---- Lua: script data (§2, §4.2) ----
    /// The length of the global `advisors` table; `None` when it is not a
    /// table.
    fn advisor_count(&self) -> Option<u32>;
    /// `advisors[i]` (1-based); `None` when it is not a table.
    fn advisor_entry(&self, i: u32) -> Option<AdvisorEntry>;
    /// The length of the `advices` table.
    fn advice_count(&self) -> u32;
    /// `advices[i]` (1-based).
    fn advice_entry(&self, i: u32) -> Option<AdviceEntry>;
    /// Runs `_atrg_ = function () return (<text>); end` and keeps a
    /// reference to `_atrg_` if it is a function (§4.5 step 3).
    fn compile_trigger(&self, text: &[u8]) -> Option<i32>;
    /// Releases a compiled trigger reference.
    fn release_trigger(&self, r: i32);
    /// Calls a compiled trigger (protected, 0 arguments, 1 result).
    fn call_trigger(&self, r: i32) -> LuaResult;
    /// Calls the `condition` function of `advices[i]` (protected, §6.6).
    fn call_condition(&self, table_index: i32) -> LuaResult;
    /// The detokenizer applied to a condition's string (§6.6).
    fn detokenize(&self, text: &[u8]) -> Vec<u8>;
    /// `reload_data` steps 1–3 (§4.2): `_on_reload`, loose-file overrides,
    /// script packages.
    fn reload_scripts(&self);
    /// `register_api(global)` (§8.2).
    fn register_api(&self, global: bool);
    /// The tutorial registry's `SetLua` (true: the system's context; false:
    /// null).
    fn set_tutorial_lua(&self, on: bool);
    /// `NewTag` twice with the gc method (§4.1 step 3): the two tags.
    fn create_tags(&self) -> (i32, i32);
    /// Registers (true) or unregisters the advice detokenizer (§4.1 step 4).
    fn register_detokenizer(&self, on: bool);
    /// `tuning_constants.ADVICE_POPUP_PRIORITIES_BY_SKILL_LEVEL[key]` as a
    /// number, if present (§6.8).
    fn popup_priority(&self, key: &str) -> Option<f64>;
    /// The forced garbage collection of the monthly step (§5.1 step 2.2).
    fn collect_garbage(&self);
    /// `Shutdown`: `game = nil`, a forced collection, the context released.
    fn close_lua(&self);
    /// `game[name] = value` (system slot 13).
    fn set_global(&self, name: &str, value: f64);
    /// `game[name] = value; game[name .. "_subject"] = subject` (slot 14).
    fn set_local(&self, name: &str, value: f64, subject: Option<&Rc<AdviceSubject>>);
    /// `game[name] = string` (slot 12).
    fn set_global_string(&self, name: &str, value: &[u8]);
    /// The app's debug functionality (`cISC4App` `+0x54`).
    fn debug_enabled(&self) -> bool;
    /// Registers / unregisters a cheat code (§4.1 step 7).
    fn register_cheat(&self, id: u32, name: &str);
    fn unregister_cheat(&self, id: u32);
    /// The advisor window (§4.1 step 10, §4.6): `start` true = release any
    /// window and `StartAdvisorWindow`; false = `RemoveWinProcs`.
    fn advisor_window(&self, start: bool);

    // ---- city and simulator ----
    fn has_city(&self) -> bool;
    /// `cISC4City` `+0x58`.
    fn city_established(&self) -> bool;
    /// `GetDifficultyLevel` (`+0x60`); `None` without a city.
    fn difficulty(&self) -> Option<i32>;
    /// `cISC4City` `+0x174` / `+0x178`.
    fn cell_counts(&self) -> (u32, u32);
    /// The simulator date (`+0x1C`).
    fn sim_date(&self) -> SimDate;
    /// The year of the simulator's start date (`+0x14`).
    fn start_year(&self) -> i32;
    /// `IsAnyPaused` (`+0x48`).
    fn is_any_paused(&self) -> bool;
    /// A millisecond clock for the idle timer (differences only).
    fn now_ms(&self) -> u32;

    // ---- messages ----
    fn subscribe(&self, target: &Target, msg_type: u32);
    fn unsubscribe(&self, target: &Target, msg_type: u32);
    /// `MessagePost(msg, 0)`.
    fn post(&self, m: &OutMessage);
    /// `MessageSend` (synchronous). Must not call back into the advisors.
    fn send(&self, m: &OutMessage);
    /// UpdateSelf to the advisor's own message target (§5.4).
    fn post_update_self(&self, advisor: &Rc<Advisor>, data1: u32, data2: u32);
    /// `tick_self` (§5.1): posts a new ticker message.
    fn post_ticker(&self);
    /// `ProposeDeals`-style side effects are the host's; this is only the
    /// Lua-visible part.
    ///
    /// The 3D view's `AssertGlobalEffectTrigger` / `Retract…` (§6.9).
    fn global_effect(&self, id: u32, assert: bool);
    /// The text rebuild of an advice (class slot 30; UI).
    fn rebuild_text(&self, advice: &Rc<Advice>);

    // ---- the variables' sources (§7) ----
    fn value(&self, src: &Src) -> Option<Val>;
    fn list(&self, src: &ListSrc) -> Option<Vec<Item>>;
    /// HistoryWarehouse `+0x30` `RecordHistoryData(type, value)` (§7.6).
    fn record_history(&self, history_type: u32, value: i32);

    // ---- occupants (§5.4, §5.5) ----
    fn occupant_type(&self, occupant: u64) -> u32;
    fn occupant_bool_property(&self, occupant: u64, id: u32) -> Option<bool>;
    /// BDS `+0x38` `GetBuildingKeyFromType(occ.GetBuildingType())`: the key
    /// instance.
    fn building_key_instance(&self, occupant: u64) -> Option<u32>;
    /// `IsOccupantGroup(group)` of an occupant (subject objects too).
    fn is_occupant_group(&self, occupant: u64, group: u32) -> bool;

    // ---- civic and rewards (§6.6) ----
    fn conditional_status(&self, guid: u32) -> Option<ConditionalStatus>;
    fn update_conditional_status(&self, guid: u32, status: &ConditionalStatus);
    /// The subject a reward builds from the building type's data (BDS
    /// `+0xA0`).
    fn reward_subject(&self, guid: u32) -> Option<SubjectInit>;

    // ---- neighbour deals (§6.3, §8.3) ----
    /// The deal type of a subject's object as `cISC4NeighborDeal`.
    fn deal_type(&self, object: u64) -> Option<u32>;
    /// `deal.Cancel()` (`+0x88`).
    fn cancel_deal(&self, object: u64);

    // ---- history, ordinances, region (§8.3) ----
    /// HistoryWarehouse `+0x20` value at `day`.
    fn history_value(&self, history_type: u32, day: i32) -> Option<f64>;
    /// `+0x18` trend slope over `[from, to]`.
    fn history_trend(&self, history_type: u32, from: i32, to: i32) -> Option<f32>;
    /// `+0x14` count over `[from, to]`.
    fn history_count(&self, history_type: u32, from: i32, to: i32) -> Option<f64>;
    /// Ordinance `+0x20` (`on` true) / `+0x1C` (`available`).
    fn ordinance_flag(&self, id: u32, on: bool) -> Option<bool>;
    fn set_ordinance_on(&self, id: u32, on: bool);
    /// `0` enact fee (`+0x2C`), `1` repeal fee (`+0x30`), `2` monthly cost
    /// (`+0x50`).
    fn ordinance_amount(&self, id: u32, which: u32) -> Option<i64>;
    /// `0` name, `1` description.
    fn ordinance_text(&self, id: u32, which: u32) -> Option<Vec<u8>>;
    /// RegionalCity `+0x104`.
    fn tutorial_guid(&self) -> Option<u32>;

    // ---- references ----
    fn add_ref(&self, token: u64);
    fn release(&self, token: u64);
    /// How a save refers to a host object (a subject's object).
    fn object_ref(&self, token: u64) -> crate::stream::RefValue;
    /// The token of a host reference read with `iid` (the reference the
    /// stream handed over is transferred).
    fn resolve_ref(&self, r: &crate::stream::RefValue, iid: u32) -> Option<u64>;
    /// A new advisor, advice or subject (`RefValue::Object`): the host's
    /// token for it (the shim's wrapper; 0 = none). Called once per object,
    /// right after construction.
    fn object_created(&self, _obj: &crate::stream::RefValue) -> u64 {
        0
    }
    /// The advisor, advice or subject with this token (from
    /// `object_created`, non-zero) is being dropped: the core will never hand
    /// the token out again. Lets the host free a wrapper that holds no
    /// strong handle on the object.
    fn object_dropped(&self, _token: u64) {}
    /// Advisor interface calls for classes supplied by another owner (§4.3,
    /// §5.3, §11.1). Operations: 0 CreateInstance(class = token), 1 Init,
    /// 2 Shutdown, 3 Simulate, 4 GetID. Creation returns an acquired token.
    fn foreign_advisor(&self, _token: u64, _operation: u32, _arg: i32) -> u64 {
        0
    }
}

/// The behaviour of an advisor class this module does not implement
/// (MySims, mysims.md §7.3): the class virtuals and the record prefix.
/// Methods get the advisor; they must call its `base_*` methods where the
/// original calls `cSC4Advisor`'s.
pub trait AdvisorExt: Any {
    /// `GetGZCLSID`.
    fn class_id(&self) -> u32;
    /// Called once when the advisor that owns this extension is created.
    fn attach(&self, _adv: &Rc<Advisor>) {}
    /// `Init(i)` (slot 3): default the base `Init`.
    fn init(&self, adv: &Rc<Advisor>, table_index: i32) -> bool {
        adv.base_init(table_index)
    }
    /// `Shutdown()` (slot 4).
    fn shutdown(&self, adv: &Rc<Advisor>) -> bool {
        adv.base_shutdown()
    }
    /// Class slot 6 `simulate_day`.
    fn simulate_day(&self, adv: &Rc<Advisor>) {
        adv.base_day_pass();
    }
    /// Class slot 7 `simulate_day_events`.
    fn simulate_day_events(&self, adv: &Rc<Advisor>) {
        adv.base_day_events();
    }
    /// Class slot 8, the monthly hook (after the month counter moved).
    fn monthly(&self, _adv: &Rc<Advisor>) {}
    /// Slot 19 `CommandWouldTrigger`.
    fn command_would_trigger(&self, adv: &Rc<Advisor>, cmd: u32) -> bool {
        adv.base_command_would_trigger(cmd)
    }
    /// `DoMessage`.
    fn do_message(&self, adv: &Rc<Advisor>, m: &AdvisorMessage) -> bool {
        adv.base_do_message(m)
    }
    /// Writes the class's own fields before the advisor record.
    fn write_prefix(&self, out: &mut dyn crate::stream::OutStream) -> bool;
    /// Reads them; false stops the read.
    fn read_prefix(&self, inp: &mut dyn crate::stream::InStream) -> bool;
    /// `QueryInterface(0x0A1DA42F)` (`cISC4MySim`).
    fn is_mysim(&self) -> bool {
        false
    }
    /// The Lua `mysim_*` functions (§8.3, mysims.md §7.2); the grades are
    /// fractions (Lua pushes them × 100).
    fn distance_to_closest(&self, _group: u32) -> Option<u32> {
        None
    }
    fn distance_to_closest_special(&self, _building_type: u32) -> Option<u32> {
        None
    }
    fn local_school_grade(&self, _group: u32) -> Option<f32> {
        None
    }
    fn local_hospital_grade(&self, _group: u32) -> Option<f32> {
        None
    }
    fn create_dispatch_tool(&self) -> Option<bool> {
        None
    }
    fn as_any(&self) -> &dyn Any;
}

/// Makes the extension of a new advisor of a registered class.
pub type AdvisorExtFactory = Rc<dyn Fn(&Rc<AdvisorSystem>) -> Rc<dyn AdvisorExt>>;
