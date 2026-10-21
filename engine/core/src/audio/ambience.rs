// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! `cSC4AudioAmbienceManager` (`docs/specs/audio-ambience.md`): the city-side
//! service that turns occupants into positional sounds, runs the occupant
//! alarm clock, keeps the listener's attention rectangle and drives the
//! ambience layers once per tick. Section references (`§n`) point at that
//! spec.
//!
//! The manager is pure state plus a typed [`AmbienceHost`]: every native
//! service it touches (framework, `cIGZSndSys`, message server, occupant and
//! lot managers, the city, the simulation clock, the audio director, the
//! audio preferences, the freshness player) is a host method. Nothing here
//! fakes a sound: a sound is a [`SndRequest`] handed to the host.
//!
//! # Partial owner
//!
//! Unsupported because the spec does not define them (open questions):
//! - the ambience layer's own behaviour (**AA1**): layers are the host's
//!   [`AmbienceLayer`] objects; the manager only calls the slots of §8;
//! - `_UpdateLotOccupantsForConditionChange` (**AA2**): the manager resolves
//!   the lot and reports [`AmbienceHost::condition_update_unspecified`]; it
//!   starts or stops nothing itself;
//! - the `RandomIterator` order: blocks are visited row-major (§5 allows any
//!   budget and requires only that every block is visited) *[choice]*.

use std::collections::BTreeSet;

/// Object size (§1).
pub const AMBIENCE_MANAGER_SIZE: usize = 0x120;
/// `kGZGUID_AudioAmbienceManagerSystemServiceId` (§1).
pub const SERVICE_ID: u32 = 0xAA12_F911;
/// Ambience layer object size (§2.2).
pub const AMBIENCE_LAYER_SIZE: usize = 0x25C;
/// Ambience layer IID (§8).
pub const IID_AMBIENCE_LAYER: u32 = 0x4A28_35FE;

/// Audio scape manager service / IID (§2.1 step 2).
pub const SCAPE_MANAGER_SERVICE: (u32, u32) = (0x49E5_0BA1, 0x29E3_9DB4);
/// `cIGZSndSys` service / IID (§2.1 step 3).
pub const SND_SYS_SERVICE: (u32, u32) = (0x23CE_CBA0, 0x1060_77D2);
/// Message server service / IID (§2.1 step 4).
pub const MESSAGE_SERVER_SERVICE: (u32, u32) = (0x04FA_845B, 0x6522_94C7);
/// Tracked-occupant set CLSID / IID (§2.1 step 5).
pub const TRACKED_SET_CLASS: (u32, u32) = (0x4A40_1BA7, 0x4A40_074C);

pub const MSG_POST_CITY_INIT: u32 = 0x26D3_1EC1;
pub const MSG_PRE_CITY_SHUTDOWN: u32 = 0x26D3_1EC2;
pub const MSG_LISTENER_POS_CHANGED: u32 = 0x8A4A_69C7;
pub const MSG_INSERT_OCCUPANT: u32 = 0x99EF_1142;
pub const MSG_MOVE_OCCUPANT: u32 = 0x99EF_1146;
pub const MSG_REMOVE_OCCUPANT: u32 = 0x99EF_1143;
pub const MSG_CONDITION_CHANGED: u32 = 0xE99B_3D4D;
pub const MSG_STATE_CHANGED: u32 = 0xE8BD_A2E7;
pub const MSG_AUTOMATA_EVENT: u32 = 0x09D9_2FC3;
pub const MSG_EXEMPLAR_CHANGED: u32 = 0xC53D_10AA;
pub const MSG_AUTOMATON_APPROACHING_INTERSECTION: u32 = 0x8A87_8385;
pub const MSG_APP_FOCUS: u32 = 0x4348_B111;

/// The 12 message types `Init` subscribes, in order (§2.1 step 4).
pub const SUBSCRIBED_MESSAGES: [u32; 12] = [
    MSG_POST_CITY_INIT,
    MSG_PRE_CITY_SHUTDOWN,
    MSG_LISTENER_POS_CHANGED,
    MSG_INSERT_OCCUPANT,
    MSG_MOVE_OCCUPANT,
    MSG_REMOVE_OCCUPANT,
    MSG_CONDITION_CHANGED,
    MSG_STATE_CHANGED,
    MSG_AUTOMATA_EVENT,
    MSG_EXEMPLAR_CHANGED,
    MSG_AUTOMATON_APPROACHING_INTERSECTION,
    MSG_APP_FOCUS,
];

/// Message payload IID every message must answer (§7).
pub const IID_MESSAGE_PAYLOAD: u32 = 0x6529_7976;
/// Occupant IID the occupant messages are QI'd with (§7).
pub const IID_OCCUPANT: u32 = 0x26BB_767E;
/// Lot IID of the condition-changed object (§3.4).
pub const IID_LOT: u32 = 0xE818_FCED;
/// Automaton IID (§3.1, §3.5).
pub const IID_AUTOMATON: u32 = 0xA9B4_0F05;
/// Freshness-player IID (§6).
pub const IID_FRESH_PLAYER: u32 = 0xDC1C_F595;

/// Sound-system events posted to `cIGZSndSys` slot `+0x2C` (§3).
pub const EVT_START_SOUND: u32 = 0xEA45_1903;
pub const EVT_STOP_SOUND: u32 = 0x6A45_1908;
pub const EVT_SET_POSITION: u32 = 0xCA45_190F;
/// Tag of the sound system's extended post (slot `+0x34`, §3.6).
pub const EXTENDED_POST_TAG: u32 = 0x5B52_3BA4;

/// Preview-occupant flag (§3).
pub const PROP_PREVIEW: u32 = 0x0292_44DB;
pub const PROP_AMBIENT_SOUND: u32 = 0x6A36_AFAC;
pub const PROP_SECOND_SOUND: u32 = 0xC9B9_3A56;
pub const PROP_ALARM_TIMES: u32 = 0x6A36_AFAB;
pub const PROP_ONE_SHOT_ALARM_SOUND: u32 = 0x0A36_AFA2;
pub const PROP_ON_OFF_ALARM_SOUND: u32 = 0x2A53_BE13;
pub const PROP_CONDITION_SOUND_A: u32 = 0xCA19_D7CA;
pub const PROP_CONDITION_SOUND_B: u32 = 0x0A4C_13CB;
pub const PROP_CONDITION_SOUND_C: u32 = 0x4A4C_132E;
pub const PROP_CROSSING_SOUND: u32 = 0xAA87_817A;

/// Occupant types of the city-init walk (§3.1; the last two are **AA4**).
pub const INIT_WALK_TYPES: [u32; 3] = [0x2781_28A0, 0xA823_821E, 0xC7CC_75F1];

/// Audio settings exemplar (§4.1).
pub const TUNING_EXEMPLAR: (u32, u32, u32) = (0x6534_284A, 0xCA25_875D, 0x6A35_9C5C);
pub const PROP_TUNING_MARGINS: u32 = 0x0A36_F410;
pub const PROP_TUNING_LAYER_IDS: u32 = 0x2A3F_F8CD;
/// Margin when `0x0A36F410` is missing (`0x428C0000`, §4.1).
pub const DEFAULT_MARGIN: f32 = 70.0;
/// Number of margins, indexed by zoom (§1 `+0x54`).
pub const MARGIN_COUNT: usize = 5;

/// `DoEvent` event ids (§6).
pub const EVT_MASTER_VOLUME: u32 = 0x2A5B_69C0;
pub const EVT_FRESH_STOP: u32 = 0x4A2C_0389;
pub const EVT_FRESH_PLAY_PERCENT: u32 = 0x0A41_4C5C;
pub const EVT_FRESH_LOAD_SCORE: u32 = 0x2A2C_0384;
pub const EVT_FRESH_FADE_OUT: u32 = 0x2A2C_0393;
pub const EVT_FRESH_SET_VOLUME: u32 = 0x4A3D_A208;
pub const EVT_FRESH_PLAY: u32 = 0xEA2C_0390;
pub const EVT_FRESH_MIN_MAX_DISTANCE: u32 = 0xEA2D_827C;

/// Automaton kinds whose sound codes 12/13 play (§3.5).
pub const AUTOMATON_SOUND_KINDS: [u32; 3] = [0x10, 0x14, 0x1E];

/// Layer-init failure log line (§2.2), `%08x` = the layer id.
pub fn layer_init_failure_message(layer_id: u32) -> String {
    format!(
        "Error : cSC4AudioAmbienceManager::_CreateAmbienceLayers : Failed to init layer ({:08x})\n",
        layer_id
    )
}

/// Freshness-score load failure log line (§6).
pub fn score_load_failure_message(t: u32, g: u32, i: u32) -> String {
    format!(
        "Error: cSC4AudioAmbienceLayer::DoEvent : Failed to load freshness score ({:08x}:{:08x}:{:08x})\n",
        t, g, i
    )
}

/// Occupant identity: the occupant's address in the host. Instance ids are
/// `occupant + sound id` (wrapping, §3).
pub type OccupantRef = u32;

/// Instance id of an occupant sound (§3).
pub fn occupant_instance_id(occupant: OccupantRef, sound_id: u32) -> u32 {
    occupant.wrapping_add(sound_id)
}

/// A sound-system event the manager posts (§3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SndRequest {
    /// `0xEA451903`.
    Start {
        sound_id: u32,
        instance_id: u32,
        looping: bool,
        position: [i32; 3],
    },
    /// `0x6A451908`.
    Stop { instance_id: u32 },
    /// `0xCA45190F`.
    SetPosition { instance_id: u32, position: [i32; 3] },
}

/// Which post a request goes through (§3, §3.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostKind {
    /// Slot `+0x2C`.
    Normal,
    /// Slot `+0x34`, tag [`EXTENDED_POST_TAG`].
    Extended,
}

/// What the manager needs to know about one occupant, read by the host from
/// its property holder (slot `+0x18`) and position (slot `+0x20`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OccupantInfo {
    pub occupant: OccupantRef,
    /// `0x029244DB` true.
    pub preview: bool,
    /// Answers [`IID_AUTOMATON`].
    pub is_automaton: bool,
    /// Position (f32 in the game; truncated to integers by the manager).
    pub position: [f32; 3],
    pub ambient_sound: Option<u32>,
    pub second_sound: Option<u32>,
    pub alarm_times: Option<Vec<f32>>,
    pub one_shot_alarm_sound: Option<u32>,
    pub on_off_alarm_sound: Option<u32>,
    pub condition_sound_a: Option<u32>,
    pub condition_sound_b: Option<u32>,
    pub condition_sound_c: Option<u32>,
    pub crossing_sound: Option<u32>,
}

impl OccupantInfo {
    fn int_position(&self) -> [i32; 3] {
        truncate(self.position)
    }
}

/// f32 → integer truncation of a position (§3).
pub fn truncate(p: [f32; 3]) -> [i32; 3] {
    [p[0] as i32, p[1] as i32, p[2] as i32]
}

/// Tuning read from the audio settings exemplar (§4.1).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AmbienceTuning {
    /// `0x0A36F410`; `None` = missing.
    pub margins: Option<Vec<f32>>,
    /// `0x2A3FF8CD`; `None` = missing.
    pub layer_ids: Option<Vec<u32>>,
}

/// A cell rectangle: x0, z0, x1, z1 (§1 `+0x6C`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CellRect {
    pub x0: i32,
    pub z0: i32,
    pub x1: i32,
    pub z1: i32,
}

/// A world rectangle (§1 `+0x8C..+0xA8`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WorldRect {
    pub x0: f32,
    pub z0: f32,
    pub x1: f32,
    pub z1: f32,
}

/// One call made on the freshness player (§6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FreshCall {
    /// Slot 10.
    Stop,
    /// Slot 7.
    Shutdown,
    /// Slot 16, raw event value.
    SetPlayPercent(u32),
    /// Slot 12: `FadeVolume(3000, 0, 1)`.
    FadeVolume { ms: u32, target: u32, flag: u32 },
    /// Slot 11, raw event value.
    SetVolume(u32),
    /// Slot 28.
    SetHeadRelative(bool),
    /// Slot 8.
    Play,
    /// Slot 29, the payload's two raw values.
    SetMinMaxDistance(u32, u32),
}

/// A sound-system event delivered to the manager's `DoEvent` (§6).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SndEventIn {
    pub id: u32,
    /// The event's value (raw 32-bit word).
    pub value: u32,
    /// Second payload value (`0xEA2D827C`).
    pub value2: u32,
    /// The data object (slot `+0x18`, then `+0x80`) answers [`IID_FRESH_PLAYER`].
    pub fresh_player: bool,
    /// Payload type/group/instance (`0x2A2C0384`); `None` = the payload does
    /// not answer [`IID_MESSAGE_PAYLOAD`].
    pub score_tgi: Option<(u32, u32, u32)>,
}

/// An automaton event message `0x09D92FC3` (§3.5).
#[derive(Debug, Clone, PartialEq)]
pub struct AutomatonEvent {
    /// Message slot `+0x38`.
    pub code: u32,
    /// Message slot `+0x4C`.
    pub kind: u32,
    /// The automaton's slot `+0xE4(0x10)`.
    pub slot_e4_0x10: u32,
    /// The automaton's occupant.
    pub occupant: OccupantInfo,
}

/// The messages `DoMessage` dispatches (§7). Each carries the inputs the host
/// resolved from the message object.
#[derive(Debug, Clone, PartialEq)]
pub enum AmbienceMessage {
    PostCityInit,
    PreCityShutdown,
    ListenerPosChanged,
    ExemplarChanged,
    AppFocus {
        flag: bool,
    },
    /// `occupant` is `None` when the message object does not answer
    /// [`IID_OCCUPANT`].
    Occupant {
        msg_type: u32,
        occupant: Option<OccupantInfo>,
    },
    /// `lot` is `None` when the object does not answer [`IID_LOT`].
    ConditionChanged {
        lot: Option<u32>,
        values: (u32, u32),
    },
    AutomataEvent(AutomatonEvent),
    /// `automaton` is `None` when the object does not answer [`IID_AUTOMATON`].
    ApproachingIntersection {
        automaton: Option<OccupantInfo>,
    },
    Other(u32),
}

/// One ambience layer as the manager calls it (§8). Behaviour is **AA1**.
pub trait AmbienceLayer {
    /// Slot 4.
    fn init(&mut self) -> bool;
    /// Slot 17 (raw volume word).
    fn set_score_master_volume(&mut self, volume: u32);
    /// Slot 20.
    fn handle_cell(&mut self, x: i32, z: i32);
    /// Slot 21, through the cell walk callback.
    fn handle_occupant(&mut self, occupant: OccupantRef);
    /// Slot 25 with the attention rectangle.
    fn begin_update(&mut self, rect: CellRect);
    /// Slot 26.
    fn update(&mut self, elapsed_ms: u32, listener_pos: [f32; 3], listener_vec: [f32; 3]);
    /// Slot 27.
    fn end_update(&mut self);
    /// Slot 28.
    fn pause(&mut self, flag: bool);
}

/// The native services the manager uses.
pub trait AmbienceHost {
    type Layer: AmbienceLayer;

    // ---- Init / Shutdown (§2.1, §2.4)
    /// Resolve the framework and call its slot 3; false = absent.
    fn framework_init(&mut self) -> bool;
    fn acquire_scape_manager(&mut self) -> bool;
    fn acquire_snd_sys(&mut self) -> bool;
    /// Sound-system slot `+0x3C` / `+0x40`.
    fn register_event_target(&mut self);
    fn unregister_event_target(&mut self);
    /// False = no message server.
    fn acquire_message_server(&mut self) -> bool;
    fn subscribe(&mut self, msg_type: u32);
    fn unsubscribe(&mut self, msg_type: u32);
    fn create_tracked_set(&mut self) -> bool;
    fn release_services(&mut self);
    /// Framework tick list (slots `+0x24` / `+0x28`).
    fn add_to_tick_list(&mut self);
    fn remove_from_tick_list(&mut self);

    // ---- sound system
    fn post(&mut self, kind: PostKind, request: SndRequest);
    /// `cSC4AudioSoundManager::GenerateNewInstanceId` (audio.md §8.4).
    fn generate_instance_id(&mut self) -> u32;

    // ---- tuning, preferences, layers
    fn read_tuning(&mut self) -> AmbienceTuning;
    /// Audio preferences: (byte `+4` set, volume word); `None` = absent.
    fn audio_preferences(&mut self) -> Option<(bool, u32)>;
    /// Allocate a layer for an exemplar id (not yet `Init`ed).
    fn create_layer(&mut self, layer_id: u32) -> Self::Layer;
    fn log(&mut self, line: &str);

    // ---- city / view / listener
    /// City cell counts (slots `+0x174`, `+0x178`).
    fn city_cell_counts(&mut self) -> (i32, i32);
    /// Occupant-manager slot `+0x48`.
    fn cell_to_world(&mut self, x: i32, z: i32) -> (f32, f32);
    /// Occupant-manager slot `+0x4C`.
    fn world_to_cell(&mut self, x: f32, z: f32) -> (i32, i32);
    /// View-utilities `+0x0C`.
    fn zoom_index(&mut self) -> u32;
    /// Audio director listener: (position words 9-11, vector words 3-5).
    fn listener(&mut self) -> ([f32; 3], [f32; 3]);

    // ---- occupants and lots
    /// Occupant-manager slot `+0xB0` with a type filter.
    fn occupants_of_types(&mut self, types: &[u32]) -> Vec<OccupantInfo>;
    /// Occupant-manager slot `+0xBC` with the filter, one cell.
    fn occupants_in_cell(&mut self, x: i32, z: i32) -> Vec<OccupantRef>;
    /// Lot manager slot `+0x38`.
    fn lot_of_occupant(&mut self, occupant: OccupantRef) -> Option<u32>;
    /// **AA2**: the per-lot condition update is unspecified; the host gets
    /// the request and the manager posts nothing for it.
    fn condition_update_unspecified(&mut self, lot: u32, values: Option<(u32, u32)>);
    /// Automaton slot `+0x9C` / `+0xA0`.
    fn link_automaton(&mut self, occupant: OccupantRef);
    fn unlink_automaton(&mut self, occupant: OccupantRef);
    /// Current position of an alarm occupant (slot `+0x20`).
    fn occupant_position(&mut self, occupant: OccupantRef) -> [f32; 3];
    /// Re-read `0x2A53BE13` (§3.6 on/off).
    fn on_off_sound_property(&mut self, occupant: OccupantRef) -> Option<u32>;

    // ---- clock / freshness
    /// Simulation clock (hour, minute); `None` = absent.
    fn clock(&mut self) -> Option<(u32, u32)>;
    fn fresh_player_call(&mut self, call: FreshCall);
    /// Load a freshness score; false = failure.
    fn fresh_player_load_score(&mut self, t: u32, g: u32, i: u32) -> bool;
}

/// Alarm kind byte `+0x25` (§3.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlarmKind {
    OneShot = 0,
    OnOff = 1,
}

/// One alarm entry (§3.6).
#[derive(Debug, Clone, PartialEq)]
pub struct AlarmEntry {
    pub occupant: OccupantRef,
    pub sound_id: u32,
    /// `+0x10`. Initial value unspecified (**AA5**): 0.0 *[choice]*.
    pub last: f32,
    pub times: Vec<f32>,
    /// `+0x24`.
    pub on: bool,
    pub kind: AlarmKind,
}

/// `OnTick` stage `+0xB0` (§5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Begin0,
    Begin1,
    Walk,
    End,
}

/// The manager state (§1).
pub struct AmbienceManager<L> {
    pub ref_count: u32,
    pub running: bool,
    pub layers: Vec<L>,
    pub zoom: u32,
    pub listener_pos: [f32; 3],
    pub listener_vec: [f32; 3],
    pub alarms: Vec<AlarmEntry>,
    /// `+0x50`.
    pub ticking: bool,
    /// `+0x54`; `None` until the tuning is read.
    pub margins: Option<[f32; MARGIN_COUNT]>,
    pub layer_ids: Vec<u32>,
    pub current: CellRect,
    pub previous: CellRect,
    pub attention_world: WorldRect,
    pub city_world: WorldRect,
    /// `+0xAC`.
    pub city_ready: bool,
    pub stage: Stage,
    /// Remaining blocks of the iterator (front = next).
    blocks: Vec<(i32, i32)>,
    block_pos: usize,
    /// `+0x118` (`None` until created) and `+0x11C`.
    pub tracked: Option<BTreeSet<OccupantRef>>,
    pub tracked_count: i32,
    pub cell_counts: (i32, i32),
    /// The global start-up byte `0x00B09824` (process-wide in the game; one
    /// per manager here) *[choice]*.
    pub first_rect_update: bool,
}

impl<L: AmbienceLayer> Default for AmbienceManager<L> {
    fn default() -> Self {
        Self::new()
    }
}

impl<L: AmbienceLayer> AmbienceManager<L> {
    /// Constructor (§1 initial values).
    pub fn new() -> Self {
        Self {
            ref_count: 0,
            running: false,
            layers: Vec::new(),
            zoom: 0,
            listener_pos: [0.0; 3],
            listener_vec: [0.0; 3],
            alarms: Vec::new(),
            ticking: true,
            margins: None,
            layer_ids: Vec::new(),
            current: CellRect::default(),
            previous: CellRect::default(),
            attention_world: WorldRect::default(),
            city_world: WorldRect::default(),
            city_ready: false,
            stage: Stage::Begin0,
            blocks: Vec::new(),
            block_pos: 0,
            tracked: None,
            tracked_count: 0,
            cell_counts: (0, 0),
            first_rect_update: true,
        }
    }

    /// `Init` (§2.1).
    pub fn init<H: AmbienceHost<Layer = L>>(&mut self, host: &mut H) -> bool {
        if !host.framework_init() || !host.acquire_scape_manager() || !host.acquire_snd_sys() {
            return false;
        }
        host.register_event_target();
        // The spec does not say what an absent message server does: the
        // literal reading of "any failure returns 0" *[choice]*.
        if !host.acquire_message_server() {
            return false;
        }
        for t in SUBSCRIBED_MESSAGES {
            host.subscribe(t);
        }
        if !host.create_tracked_set() {
            return false;
        }
        self.tracked = Some(BTreeSet::new());
        self.ticking = true;
        self.running = true;
        true
    }

    /// `Shutdown` (§2.4). The unsubscribe order differs in the game; the set
    /// is the same and the order is not given, so §2.1's order is used.
    pub fn shutdown<H: AmbienceHost<Layer = L>>(&mut self, host: &mut H) -> bool {
        host.unregister_event_target();
        for t in SUBSCRIBED_MESSAGES {
            host.unsubscribe(t);
        }
        self.release_layers();
        host.release_services();
        self.running = false;
        true
    }

    /// `0x004E9660`: release every layer and empty the lists (layers and
    /// alarms; "the lists" read as both) *[choice]*.
    fn release_layers(&mut self) -> bool {
        self.layers.clear();
        self.alarms.clear();
        true
    }

    /// `_ReadTuningValues` (§4.1).
    pub fn read_tuning<H: AmbienceHost<Layer = L>>(&mut self, host: &mut H) {
        let t = host.read_tuning();
        let mut m = [DEFAULT_MARGIN; MARGIN_COUNT];
        if let Some(v) = t.margins {
            // A list shorter than 5 keeps 70.0 for the rest; longer lists are
            // cut (**AA6**) *[choice]*.
            for (d, s) in m.iter_mut().zip(v) {
                *d = s;
            }
        }
        self.margins = Some(m);
        // Missing layer ids: the vector is left empty (**AA6**) *[choice]*.
        self.layer_ids = t.layer_ids.unwrap_or_default();
    }

    /// City init (§2.2).
    pub fn handle_city_init<H: AmbienceHost<Layer = L>>(&mut self, host: &mut H) -> bool {
        self.read_tuning(host);
        let ids = self.layer_ids.clone();
        let volume = match host.audio_preferences() {
            Some((true, v)) => v,
            // Absent preferences: treated as a clear byte (**AA6**) *[choice]*.
            _ => 0,
        };
        for id in ids {
            let mut layer = host.create_layer(id);
            if !layer.init() {
                host.log(&layer_init_failure_message(id));
                continue;
            }
            layer.set_score_master_volume(volume);
            self.layers.push(layer);
        }
        let (nx, nz) = host.city_cell_counts();
        self.cell_counts = (nx, nz);
        let (ax, az) = host.cell_to_world(0, 0);
        let (bx, bz) = host.cell_to_world(nx - 1, nz - 1);
        self.city_world = WorldRect {
            x0: ax,
            z0: az,
            x1: bx,
            z1: bz,
        };
        self.city_ready = true;
        self.update_attention_rect(host);
        self.reset_iterator();
        self.stage = Stage::Begin0;
        host.add_to_tick_list();
        for occ in host.occupants_of_types(&INIT_WALK_TYPES) {
            self.handle_occupant_common(host, &occ, false);
        }
        self.ticking = true;
        true
    }

    /// City shutdown (§2.3).
    pub fn handle_city_shutdown<H: AmbienceHost<Layer = L>>(&mut self, host: &mut H) -> bool {
        self.ticking = false;
        host.remove_from_tick_list();
        self.release_layers()
    }

    /// `_UpdateAttentionRect` (§4.2).
    pub fn update_attention_rect<H: AmbienceHost<Layer = L>>(&mut self, host: &mut H) {
        let (pos, vec) = host.listener();
        self.listener_pos = pos;
        self.listener_vec = vec;
        self.zoom = host.zoom_index();
        // "no bounds check" in the game; an index past the five margins (or
        // margins never read) has no defined value here, so the rectangle is
        // left unchanged (**AA6**) *[choice]*.
        let Some(m) = self.margins.and_then(|ms| ms.get(self.zoom as usize).copied()) else {
            return;
        };
        let (x, z) = (pos[0], pos[2]);
        self.attention_world = WorldRect {
            x0: x - m,
            z0: z - m,
            x1: x + m,
            z1: z + m,
        };
        let (cx0, cz0) = host.world_to_cell(x - m, z - m);
        let (cx1, cz1) = host.world_to_cell(x + m, z + m);
        let (nx, nz) = self.cell_counts;
        let fix = |v: i32, n: i32| v.clamp(0, (n - 1).max(0)) & !1;
        let r = CellRect {
            x0: fix(cx0, nx),
            z0: fix(cz0, nz),
            x1: fix(cx1, nx),
            z1: fix(cz1, nz),
        };
        if self.first_rect_update {
            self.previous = r;
            self.first_rect_update = false;
        } else {
            self.previous = self.current;
        }
        self.current = r;
    }

    fn reset_iterator(&mut self) {
        self.blocks.clear();
        self.block_pos = 0;
    }

    /// Seed the iterator with the current rectangle halved (§5). The bounds
    /// are read inclusive (`x0/2 ..= x1/2`) and the order row-major (z outer)
    /// (**AA7**) *[choice]*.
    fn seed_iterator(&mut self) {
        self.blocks.clear();
        self.block_pos = 0;
        let r = self.current;
        for bz in (r.z0 / 2)..=(r.z1 / 2) {
            for bx in (r.x0 / 2)..=(r.x1 / 2) {
                self.blocks.push((bx, bz));
            }
        }
    }

    /// Blocks the seeded iterator visits.
    pub fn seeded_blocks(&self) -> &[(i32, i32)] {
        &self.blocks
    }

    /// `OnTick` (§5). `elapsed_ms` is the tick timer's elapsed time;
    /// `budget` answers "may another block be taken" before each block
    /// (§5 allows any budget) *[choice]*.
    pub fn on_tick<H: AmbienceHost<Layer = L>>(
        &mut self,
        host: &mut H,
        elapsed_ms: u32,
        mut budget: impl FnMut() -> bool,
    ) {
        if !self.ticking {
            return;
        }
        self.update_alarms(host);
        if matches!(self.stage, Stage::Begin0 | Stage::Begin1) {
            self.seed_iterator();
            let r = self.current;
            for l in &mut self.layers {
                l.begin_update(r);
            }
            self.stage = Stage::Walk;
        }
        if self.stage == Stage::Walk && self.do_stage_update(host, elapsed_ms, &mut budget) {
            self.stage = Stage::End;
        }
        if self.stage == Stage::End {
            for l in &mut self.layers {
                l.end_update();
            }
            self.stage = Stage::Begin1;
        }
    }

    /// `_DoStageUpdate` (§5): returns true when the walk finished.
    fn do_stage_update<H: AmbienceHost<Layer = L>>(
        &mut self,
        host: &mut H,
        elapsed_ms: u32,
        budget: &mut dyn FnMut() -> bool,
    ) -> bool {
        let mut finished = false;
        while budget() {
            let Some(&(bx, bz)) = self.blocks.get(self.block_pos) else {
                finished = true;
                break;
            };
            self.block_pos += 1;
            let cells = [
                (2 * bx, 2 * bz),
                (2 * bx + 1, 2 * bz),
                (2 * bx, 2 * bz + 1),
                (2 * bx + 1, 2 * bz + 1),
            ];
            for &(x, z) in &cells {
                for occ in host.occupants_in_cell(x, z) {
                    for l in &mut self.layers {
                        l.handle_occupant(occ);
                    }
                }
            }
            for &(x, z) in &cells {
                for l in &mut self.layers {
                    l.handle_cell(x, z);
                }
            }
        }
        let (p, v) = (self.listener_pos, self.listener_vec);
        for l in &mut self.layers {
            l.update(elapsed_ms, p, v);
        }
        finished
    }

    // ---- occupants (§3)

    fn start_occupant_sound<H: AmbienceHost<Layer = L>>(
        &mut self,
        host: &mut H,
        occ: &OccupantInfo,
        sound_id: u32,
        looping: bool,
    ) {
        if occ.is_automaton {
            host.link_automaton(occ.occupant);
        }
        let instance_id = occupant_instance_id(occ.occupant, sound_id);
        let position = occ.int_position();
        host.post(
            PostKind::Normal,
            SndRequest::Start {
                sound_id,
                instance_id,
                looping,
                position,
            },
        );
        host.post(PostKind::Normal, SndRequest::SetPosition { instance_id, position });
    }

    fn stop_occupant_sound<H: AmbienceHost<Layer = L>>(&mut self, host: &mut H, occ: &OccupantInfo, sound_id: u32) {
        if occ.is_automaton {
            host.unlink_automaton(occ.occupant);
        }
        host.post(
            PostKind::Normal,
            SndRequest::Stop {
                instance_id: occupant_instance_id(occ.occupant, sound_id),
            },
        );
    }

    fn set_position<H: AmbienceHost<Layer = L>>(host: &mut H, occ: &OccupantInfo, sound_id: u32) {
        host.post(
            PostKind::Normal,
            SndRequest::SetPosition {
                instance_id: occupant_instance_id(occ.occupant, sound_id),
                position: occ.int_position(),
            },
        );
    }

    /// Insert (`insert = true`) and the init walk (§3.1).
    fn handle_occupant_common<H: AmbienceHost<Layer = L>>(&mut self, host: &mut H, occ: &OccupantInfo, insert: bool) {
        if occ.preview {
            return;
        }
        if let Some(set) = self.tracked.as_mut() {
            if set.insert(occ.occupant) {
                self.tracked_count += 1;
            }
        }
        if insert {
            if let Some(s) = occ.ambient_sound {
                self.start_occupant_sound(host, occ, s, true);
            }
        }
        if let Some(times) = &occ.alarm_times {
            if let Some(s) = occ.one_shot_alarm_sound {
                self.add_alarm(occ.occupant, s, times, AlarmKind::OneShot);
            }
            if let Some(s) = occ.on_off_alarm_sound {
                self.add_alarm(occ.occupant, s, times, AlarmKind::OnOff);
            }
        }
        if insert {
            if let Some(s) = occ.second_sound {
                // "start that sound once": the same start + set-position as
                // the ambient sound, non-looping *[choice]* (**AA8**).
                self.start_occupant_sound(host, occ, s, false);
            }
        }
        if occ.condition_sound_a.is_some() || occ.condition_sound_b.is_some() || occ.condition_sound_c.is_some() {
            if let Some(lot) = host.lot_of_occupant(occ.occupant) {
                host.condition_update_unspecified(lot, None);
            }
        }
    }

    fn add_alarm(&mut self, occupant: OccupantRef, sound_id: u32, times: &[f32], kind: AlarmKind) {
        self.alarms.push(AlarmEntry {
            occupant,
            sound_id,
            last: 0.0,
            times: times.to_vec(),
            on: false,
            kind,
        });
    }

    /// `_RemoveAlarmOccupant`: the entry of this occupant and kind.
    fn remove_alarm(&mut self, occupant: OccupantRef, kind: AlarmKind) {
        if let Some(i) = self
            .alarms
            .iter()
            .position(|a| a.occupant == occupant && a.kind == kind)
        {
            self.alarms.remove(i);
        }
    }

    /// Move (§3.2).
    fn handle_move<H: AmbienceHost<Layer = L>>(&mut self, host: &mut H, occ: &OccupantInfo) {
        if occ.preview {
            return;
        }
        if let Some(s) = occ.ambient_sound {
            Self::set_position(host, occ, s);
            // The condition sound's instance id is `occupant + 0xCA19D7CA's
            // value` (**AA8**) *[choice]*.
            if let Some(c) = occ.condition_sound_a {
                Self::set_position(host, occ, c);
            }
        }
    }

    /// Remove (§3.3).
    fn handle_remove<H: AmbienceHost<Layer = L>>(&mut self, host: &mut H, occ: &OccupantInfo) {
        if occ.preview {
            return;
        }
        if let Some(set) = self.tracked.as_mut() {
            if set.remove(&occ.occupant) {
                self.tracked_count -= 1;
            }
        }
        for s in [occ.condition_sound_b, occ.condition_sound_a, occ.ambient_sound]
            .into_iter()
            .flatten()
        {
            self.stop_occupant_sound(host, occ, s);
        }
        if occ.one_shot_alarm_sound.is_some() {
            self.remove_alarm(occ.occupant, AlarmKind::OneShot);
        }
        if occ.on_off_alarm_sound.is_some() {
            self.remove_alarm(occ.occupant, AlarmKind::OnOff);
        }
    }

    /// Automaton event (§3.5).
    fn handle_automaton<H: AmbienceHost<Layer = L>>(&mut self, host: &mut H, ev: &AutomatonEvent) {
        let occ = &ev.occupant;
        // Without `0x6A36AFAC` there is no sound to play (**AA8**) *[choice]*.
        let Some(s) = occ.ambient_sound else {
            return;
        };
        match ev.code {
            9 => Self::set_position(host, occ, s),
            12 | 13 if AUTOMATON_SOUND_KINDS.contains(&ev.kind) => {
                let start = ev.code == 13 && (ev.kind != 0x10 || ev.slot_e4_0x10 == 0);
                if start {
                    let instance_id = occupant_instance_id(occ.occupant, s);
                    let position = occ.int_position();
                    host.post(
                        PostKind::Normal,
                        SndRequest::Start {
                            sound_id: s,
                            instance_id,
                            looping: false,
                            position,
                        },
                    );
                    host.post(PostKind::Normal, SndRequest::SetPosition { instance_id, position });
                } else {
                    host.post(
                        PostKind::Normal,
                        SndRequest::Stop {
                            instance_id: occupant_instance_id(occ.occupant, s),
                        },
                    );
                }
            }
            _ => {}
        }
    }

    /// Railway crossing (§3.5).
    fn handle_crossing<H: AmbienceHost<Layer = L>>(&mut self, host: &mut H, occ: &OccupantInfo) {
        let Some(s) = occ.crossing_sound else {
            return;
        };
        let instance_id = occupant_instance_id(occ.occupant, s);
        let position = occ.int_position();
        host.post(
            PostKind::Normal,
            SndRequest::Start {
                sound_id: s,
                instance_id,
                looping: false,
                position,
            },
        );
        host.post(PostKind::Normal, SndRequest::SetPosition { instance_id, position });
    }

    /// `_UpdateAlarms` (§3.6).
    pub fn update_alarms<H: AmbienceHost<Layer = L>>(&mut self, host: &mut H) {
        let Some((hour, minute)) = host.clock() else {
            return;
        };
        let t = time_of_day(hour, minute);
        let mut alarms = std::mem::take(&mut self.alarms);
        for a in &mut alarms {
            let n = a.times.len();
            for i in 0..n {
                let ai = a.times[i];
                let next = a.times[(i + 1) % n];
                match a.kind {
                    AlarmKind::OneShot => {
                        if a.last < ai && ai <= t {
                            let instance_id = host.generate_instance_id();
                            let position = truncate(host.occupant_position(a.occupant));
                            host.post(
                                PostKind::Extended,
                                SndRequest::Start {
                                    sound_id: a.sound_id,
                                    instance_id,
                                    looping: false,
                                    position,
                                },
                            );
                            host.post(PostKind::Extended, SndRequest::SetPosition { instance_id, position });
                        }
                    }
                    AlarmKind::OnOff => {
                        let inside =
                            (ai <= t && t < next) || (next < ai && t >= ai) || (t - 24.0 >= ai && t - 24.0 < next);
                        if !inside {
                            continue;
                        }
                        // The property is re-read; when it is gone nothing
                        // happens (**AA8**) *[choice]*.
                        let Some(s) = host.on_off_sound_property(a.occupant) else {
                            continue;
                        };
                        // Instance id `occupant + sound id`, like every other
                        // occupant loop (**AA8**) *[choice]*.
                        let instance_id = occupant_instance_id(a.occupant, s);
                        if i % 2 == 0 {
                            if !a.on {
                                let position = truncate(host.occupant_position(a.occupant));
                                host.post(
                                    PostKind::Normal,
                                    SndRequest::Start {
                                        sound_id: s,
                                        instance_id,
                                        looping: true,
                                        position,
                                    },
                                );
                                a.on = true;
                            }
                        } else if a.on {
                            host.post(PostKind::Normal, SndRequest::Stop { instance_id });
                            a.on = false;
                        }
                    }
                }
            }
            a.last = t;
        }
        self.alarms = alarms;
    }

    // ---- events (§6) and messages (§7)

    /// `DoEvent` (§6): true when handled.
    pub fn do_event<H: AmbienceHost<Layer = L>>(&mut self, host: &mut H, ev: &SndEventIn) -> bool {
        if ev.id == EVT_MASTER_VOLUME {
            for l in &mut self.layers {
                l.set_score_master_volume(ev.value);
            }
            return true;
        }
        if !ev.fresh_player {
            return false;
        }
        match ev.id {
            EVT_FRESH_STOP => {
                host.fresh_player_call(FreshCall::Stop);
                host.fresh_player_call(FreshCall::Shutdown);
            }
            EVT_FRESH_PLAY_PERCENT => host.fresh_player_call(FreshCall::SetPlayPercent(ev.value)),
            EVT_FRESH_LOAD_SCORE => {
                // A payload that does not answer 0x65297976: not handled
                // (**AA9**) *[choice]*.
                let Some((t, g, i)) = ev.score_tgi else {
                    return false;
                };
                if !host.fresh_player_load_score(t, g, i) {
                    host.log(&score_load_failure_message(t, g, i));
                }
            }
            EVT_FRESH_FADE_OUT => host.fresh_player_call(FreshCall::FadeVolume {
                ms: 3000,
                target: 0,
                flag: 1,
            }),
            EVT_FRESH_SET_VOLUME => host.fresh_player_call(FreshCall::SetVolume(ev.value)),
            EVT_FRESH_PLAY => {
                host.fresh_player_call(FreshCall::SetHeadRelative(ev.value != 0));
                host.fresh_player_call(FreshCall::Play);
            }
            EVT_FRESH_MIN_MAX_DISTANCE => host.fresh_player_call(FreshCall::SetMinMaxDistance(ev.value, ev.value2)),
            _ => return false,
        }
        true
    }

    /// `DoMessage` (§7). `is_payload`: the message answers
    /// [`IID_MESSAGE_PAYLOAD`].
    pub fn do_message<H: AmbienceHost<Layer = L>>(
        &mut self,
        host: &mut H,
        is_payload: bool,
        msg: &AmbienceMessage,
    ) -> bool {
        if !is_payload {
            return false;
        }
        match msg {
            AmbienceMessage::PostCityInit => self.handle_city_init(host),
            AmbienceMessage::PreCityShutdown => self.handle_city_shutdown(host),
            AmbienceMessage::AppFocus { flag } => {
                for l in &mut self.layers {
                    l.pause(*flag);
                }
                // The focus handler's result is not given (**AA9**): 1.
                true
            }
            AmbienceMessage::ListenerPosChanged => {
                self.update_attention_rect(host);
                true
            }
            AmbienceMessage::ExemplarChanged => {
                self.read_tuning(host);
                true
            }
            AmbienceMessage::AutomataEvent(ev) => {
                self.handle_automaton(host, ev);
                true
            }
            AmbienceMessage::ApproachingIntersection { automaton } => {
                if let Some(a) = automaton {
                    self.handle_crossing(host, a);
                }
                true
            }
            AmbienceMessage::Occupant { msg_type, occupant } => {
                if let Some(o) = occupant {
                    match *msg_type {
                        MSG_INSERT_OCCUPANT => self.handle_occupant_common(host, o, true),
                        MSG_MOVE_OCCUPANT => self.handle_move(host, o),
                        MSG_REMOVE_OCCUPANT => self.handle_remove(host, o),
                        // StateChanged: no case *[quirk]*.
                        _ => {}
                    }
                }
                true
            }
            AmbienceMessage::ConditionChanged { lot, values } => {
                if let Some(lot) = lot {
                    host.condition_update_unspecified(*lot, Some(*values));
                }
                true
            }
            AmbienceMessage::Other(_) => true,
        }
    }
}

/// Time of day (§3.6): `float(hour) + float(minute / 60)` with an unsigned
/// integer division, so the fraction is 0 for minutes 0-59 *[quirk]*.
pub fn time_of_day(hour: u32, minute: u32) -> f32 {
    hour as f32 + (minute / 60) as f32
}

#[cfg(test)]
#[path = "ambience_tests.rs"]
mod tests;
