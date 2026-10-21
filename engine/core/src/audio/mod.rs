// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The audio layer (`docs/specs/audio.md`): the sound-event system, the sound
//! managers and the sound data. Section references (`§n`) point at that spec.
//!
//! # Structure
//!
//! - [`message`]: `cRZMessage2Standard`, the 0x2C-byte message payload and its
//!   presence-flag contract (§3.2).
//! - [`snd_event`]: `cGZSndEventStd`, the pooled sound-event object and its
//!   `+0x2C` payload interface (§3.1, §5).
//! - [`pool`]: the sound-event pool of §4 — 16 KB pages, the LIFO free list,
//!   the `0x40000000` marker and the `0x40000045` threshold. The run-9 crash
//!   site `0x005E16F5` is the free-list pop of `FUN_005E16E0` (§4.3).
//! - [`instance_id`]: `cSC4AudioSoundManager::GenerateNewInstanceId` (§8.4).
//! - [`automaton`]: the automaton's SFX slots and the messages they post
//!   (§3.3, §6).
//! - [`event_handler`]: `cSC4AudioEventHandler` (§7).
//! - [`sound_manager`]: `cSC4AudioSoundManager` and `tSound` (§8).
//! - [`effect_component`]: the effects system's sound component (§9).
//! - [`leaf`]: the 83 leaf classes of the audio director (§11).
//! - [`director`]: `cSC4AudioCOMDirector` and its registration order (§1.1).
//! - [`data`]: `Sound.dat`, the audio configuration record, AEMS banks and
//!   tuning (§10).
//! - [`save`]: nothing in the audio layer is saved (§12).
//! - [`ambience`]: `cSC4AudioAmbienceManager` (audio-ambience.md).
//!
//! # Choices made where the spec is open (`[choice]`)
//!
//! The spec leaves **AU1, AU5, AU6, AU8–AU12** unsettled (§15). They are
//! numbered questions in the lane report and are marked `[choice]` in the code
//! at the point of decision:
//!
//! 1. **AU1** — where the `"AB"` magic of an AEMS bank is checked. The spec
//!    pins the header's structural validation to the version byte at `+0x02`
//!    and says no magic check is visible in `LoadFromFile` or
//!    `SNDAEMSI_addmodulebank`. [`data`] therefore **does not** require the
//!    magic to load a bank and treats a bad magic as a caller-side error.
//! 2. **AU5** — the role of the 83 leaf classes. Their shapes, sizes, IIDs and
//!    capability slots are measured; what they *do* is not. [`leaf`]
//!    implements the measured surface only (sizes, QI, Release, the two
//!    always-true predicates) and leaves slot 5 unimplemented.
//! 3. **AU6** — what the pool's `0x40000045` threshold means. [`pool`]
//!    reproduces the measured arithmetic exactly (`add 0x40000000` on entry,
//!    `inc` on alloc, `dec` on release, and the two post-`dec` branches) and
//!    implements the coherent reading of §4.2: the low bits are the live-object
//!    count and `0x40000000` marks the page in the active ring. The initial
//!    value `FUN_0090CF9A`'s successor writes to `page+0x0C` is **0** here.
//! 4. **AU8** — Hit tracks, the radio and the freshness score are boundaries
//!    only (§2, §11). They are **out of scope**: this module does not
//!    implement them.
//! 5. **AU9** — the `0xFF000000` tag in `cGZSndMgr::GetSound` (§8.6).
//!    [`sound_manager`] applies the OR before the lookup, as measured.
//! 6. **AU10** — the CLSID `0x00436EB4` registered for type `0x2026960B`
//!    (§10.5). [`data`] records the constant; it is not resolved to a name.
//! 7. **AU11** — the `.tlo` entry layout is variable-stride (32–40 bytes)
//!    (§10.11). [`data`] parses the 16-byte header and treats the entries as
//!    opaque, stride unknown.
//! 8. **AU12** — property-key *names* for the audio exemplar keys (§10.8) are
//!    unrecovered. [`data`] keys everything by the raw 32-bit ids.
//!
//! # Threads
//!
//! The game calls getters from a second thread while the main thread is inside
//! `OnTick` (HANDOFF §5.1), so shared state a getter reads must not live in a
//! `RefCell`. The core keeps its mutable state in `RefCell`s and the capi layer
//! publishes a snapshot behind a `Mutex` for the getters, exactly as the other
//! cores do.

/// `cSC4AudioAmbienceManager` (`docs/specs/audio-ambience.md`).
pub mod ambience;
pub mod automaton;
pub mod data;
pub mod director;
pub mod effect_component;
pub mod event_handler;
pub mod instance_id;
pub mod leaf;
pub mod message;
pub mod pool;
pub mod save;
pub mod snd_event;
pub mod sound_manager;

// ---- class ids (§1.2) -------------------------------------------------------

/// `cSC4AudioSoundManager`, CLSID `0x4A47DC0D` (§1.2, §8). Object size 128.
pub const CLSID_SOUND_MANAGER: u32 = 0x4A47_DC0D;
/// `cSC4AudioEventHandler`, CLSID `0xBDB6171E` (§1.2, §7). Object size 80.
pub const CLSID_EVENT_HANDLER: u32 = 0xBDB6_171E;
/// `cSC4AudioEventHandlerDisasterFire`, CLSID `0x69E500BC` (§1.2, §7.4).
pub const CLSID_EVENT_HANDLER_DISASTER_FIRE: u32 = 0x69E5_00BC;
/// `cSC4AudioSemManager`, CLSID `0xAA355871` (§1.2).
pub const CLSID_SEM_MANAGER: u32 = 0xAA35_5871;
/// `cSC4AudioScapeManager`, CLSID `0x29E2797F` (§1.2).
pub const CLSID_SCAPE_MANAGER: u32 = 0x29E2_797F;
/// `cSC4AudioAmbienceManager`, CLSID `0x6A12F51C` (§1.2).
pub const CLSID_AMBIENCE_MANAGER: u32 = 0x6A12_F51C;
/// `cSC4AudioListener`, CLSID `0xCA4A66B9` (§1.2).
pub const CLSID_LISTENER: u32 = 0xCA4A_66B9;
/// `cGZSndSys`, CLSID `0xE41CE9FD` (§1.2).
pub const CLSID_GZ_SND_SYS: u32 = 0xE41C_E9FD;
/// `cGZSndMgr`, CLSID `0x7AF85B98` (§1.2).
pub const CLSID_GZ_SND_MGR: u32 = 0x7AF8_5B98;
/// `cRZRadio`, CLSID `0x5CFBDBBF` (§1.2).
pub const CLSID_RADIO: u32 = 0x5CFB_DBBF;
/// `cRZDiskRadioStation`, CLSID `0x5CFBDBD5` (§1.2).
pub const CLSID_DISK_RADIO_STATION: u32 = 0x5CFB_DBD5;

// ---- object sizes (§7, §8, §9, §11.1) ---------------------------------------

/// `cSC4AudioEventHandler` object size (§7): `0x50` = 80 bytes.
pub const EVENT_HANDLER_SIZE: usize = 0x50;
/// `cSC4AudioSoundManager` object size (§8): `0x80` = 128 bytes.
pub const SOUND_MANAGER_SIZE: usize = 0x80;
/// `cSC4AudioEventHandlerDisasterFire` object size (§7.4): `0x34` = 52 bytes.
pub const EVENT_HANDLER_DISASTER_FIRE_SIZE: usize = 0x34;
/// `cSC4SoundDescription` object size (§10.9): `0x1C` = 28 bytes.
pub const SOUND_DESCRIPTION_SIZE: usize = 0x1C;
/// The effects system's sound component object size (§9.1): `0x30` = 48 bytes.
pub const SOUND_COMPONENT_SIZE: usize = 0x30;

// ---- message types (§6, §7, §13) -------------------------------------------

/// `AutomataPlaySFX`, posted by `StartSFX` (§6.2, §13).
pub const MSG_AUTOMATA_PLAY_SFX: u32 = 0x8A38_5DD8;
/// `AutomataStopSFX`, posted by `StopSFX` (§6.3, §13).
pub const MSG_AUTOMATA_STOP_SFX: u32 = 0xAA39_6960;
/// Another type `HandleAutomataMessages` routes (§7.2).
pub const MSG_AUTOMATA_A: u32 = 0xA9E3_B6E3;
/// Another type `HandleAutomataMessages` routes (§7.2).
pub const MSG_AUTOMATA_B: u32 = 0xEA3A_DA33;
/// `kSC4MessagePostCityInit` — adds the city set of subscriptions (§7.2).
pub const MSG_POST_CITY_INIT: u32 = 0x26D3_1EC1;
/// `kSC4MessagePreCityShutdown` — removes it (§7.2).
pub const MSG_PRE_CITY_SHUTDOWN: u32 = 0x26D3_1EC2;
/// `kMessageListenerPosChanged` (§1.3, §8.3).
pub const MSG_LISTENER_POS_CHANGED: u32 = 0x8A4A_69C7;
/// `kMessageSetRadioStation` (§1.3, §7.2).
pub const MSG_SET_RADIO_STATION: u32 = 0x2A3A_D653;
/// `kSC4MessagePostAppInit` (§8.3).
pub const MSG_POST_APP_INIT: u32 = 0x28E9_397B;
/// `kSC4MessagePreAppShutdown` (§8.3).
pub const MSG_PRE_APP_SHUTDOWN: u32 = 0x28E9_397C;
/// `kMessageTypeAppGainLoseFocus` (§8.3).
pub const MSG_APP_FOCUS: u32 = 0x4348_B111;
/// `kSC4MessageSimSpeedChange` (§8.3).
pub const MSG_SIM_SPEED_CHANGE: u32 = 0x4A7F_B7E9;
/// `kSC4MessageSimPauseChange` (§8.3).
pub const MSG_SIM_PAUSE_CHANGE: u32 = 0xAA7F_B7E0;
/// The exemplar-changed message that re-reads tuning (§8.3, §14.18).
pub const MSG_EXEMPLAR_CHANGED: u32 = 0xC53D_10AA;

// ---- sound-system event ids (§7.3, §8.3) ------------------------------------

/// The start-sound event (`0xEA451903`), posted with the sound id and the
/// instance key (§7.3, §9.2).
pub const EVT_START_SOUND: u32 = 0xEA45_1903;
/// The stop-sound event (`0x6A451908`) (§7.3, §9.2).
pub const EVT_STOP_SOUND: u32 = 0x6A45_1908;
/// The set-sound-position event (`0xCA45190F`) (§7.3, §9.2).
pub const EVT_SET_POSITION: u32 = 0xCA45_190F;

// ---- interface ids (§1.3) ---------------------------------------------------

/// The message payload interface `HandleAutomataMessages` queries on the
/// message (§7.3).
pub const IID_MESSAGE_PAYLOAD: u32 = 0x6529_7976;
/// The position interface queried on the payload (§7.3).
pub const IID_POSITION: u32 = 0xA9B4_0F05;
/// `cIGZSndMgr` (§1.3).
pub const IID_SND_MGR: u32 = 0x3AF8_5BB7;
/// `kGZIID_cIGZSndDataSourceHolder`, the IID `GetSound` / `_StartSound` ask
/// for (§8.6, §10.6).
pub const IID_SND_DATA_SOURCE_HOLDER: u32 = 0x2060_77D2;

// ---- the automaton's SFX slots (§3.3, §6.1) ---------------------------------

/// `StartSFX` is vtable slot **61** at `+0xF4` (§6.1).
pub const SLOT_START_SFX: u32 = 61;
/// `StopSFX` is vtable slot **62** at `+0xF8` (§6.1).
pub const SLOT_STOP_SFX: u32 = 62;
/// `StartSFX`'s vtable offset (§6.1).
pub const OFFSET_START_SFX: u32 = 0xF4;
/// `StopSFX`'s vtable offset (§6.1).
pub const OFFSET_STOP_SFX: u32 = 0xF8;
/// `cSC4Automaton+0x8C` — the sound instance id (§3.3).
pub const AUTOMATON_INSTANCE_OFFSET: u32 = 0x8C;
/// `cSC4Automaton+0x84` — the automaton's flag word (§3.3).
pub const AUTOMATON_FLAGS_OFFSET: u32 = 0x84;
/// The state base record's `+0x18` — the SFX id (§3.3, §6.1).
pub const STATE_SFX_OFFSET: u32 = 0x18;

// ---- the rate limiter (§7.3, §14.5, §14.13) ---------------------------------

/// The payload value the rate limiter applies to, `0xA9F14C47` (§7.3). Every
/// other payload plays immediately *[quirk]*.
pub const RATE_LIMIT_PAYLOAD: u32 = 0xA9F1_4C47;
/// The lower bound of the rate-limit window, in ms (§7.3, §14.13).
pub const RATE_LIMIT_MIN_MS: u32 = 1000;
/// The width of the rate-limit window: `1000 + rand()*3000/0x7FFF` spans
/// `[1000, 4000]`, inclusive of 4000 (§14.13).
pub const RATE_LIMIT_SPAN: u32 = 3000;
/// The divisor in `rand()*3000/0x7FFF` (§7.3).
pub const RAND_MAX: u32 = 0x7FFF;
/// `cSC4AudioEventHandler+0x48` — the rate-limit window (§7.1).
pub const HANDLER_WINDOW_OFFSET: u32 = 0x48;
/// `cSC4AudioEventHandler+0x30` — the rate-limit timer (§7.1).
pub const HANDLER_TIMER_OFFSET: u32 = 0x30;

// ---- the sound manager (§8.1) -----------------------------------------------

/// `cSC4AudioSoundManager+0x14` — master volume, default `0x400` (§8.1).
pub const MANAGER_MASTER_VOLUME_DEFAULT: u32 = 0x400;
/// `cSC4AudioSoundManager+0x30` — focus-restore countdown, set to 50 (§8.1).
pub const MANAGER_FOCUS_COUNTDOWN: u32 = 50;

// ---- the effect component (§9.1) --------------------------------------------

/// The effects component **type** of `cSC4SoundDescription::GetType`, which is
/// `MOV EAX, 7; RET` (§9).
pub const EFFECT_TYPE_SOUND: u32 = 7;
/// The ignore bit of the component record `+0x10`: bit 0 (§9.3).
pub const IGNORE_BIT: u32 = 0x01;
/// The runtime flag bit the ignore bit becomes: `runtime+0x40 |= 4` (§9.3).
pub const RUNTIME_FLAG_IGNORED: u32 = 0x04;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::message::MESSAGE_SIZE;
    use crate::audio::snd_event::SND_EVENT_SIZE;

    /// The sizes of §3.1, §3.2, §7, §8, §9.1 must match the measured ones.
    #[test]
    fn object_sizes_match_the_spec() {
        assert_eq!(MESSAGE_SIZE, 0x2C);
        assert_eq!(SND_EVENT_SIZE, 0x38);
        assert_eq!(EVENT_HANDLER_SIZE, 0x50);
        assert_eq!(SOUND_MANAGER_SIZE, 0x80);
        assert_eq!(SOUND_COMPONENT_SIZE, 0x30);
    }

    /// The SFX slots are 61 and 62, at `+0xF4` and `+0xF8` (§6.1).
    #[test]
    fn sfx_slots_and_offsets() {
        assert_eq!(SLOT_START_SFX, 0x3D);
        assert_eq!(SLOT_STOP_SFX, 0x3E);
        assert_eq!(OFFSET_START_SFX, SLOT_START_SFX * 4);
        assert_eq!(OFFSET_STOP_SFX, SLOT_STOP_SFX * 4);
    }
}
