// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The game effect (`docs/specs/effects.md` §9), the component kind that burns,
//! demolishes, spills, extinguishes, pauses and posts messages.
//!
//! A game component is built from a **game record** (§9.1, [`shipped`]). It
//! acts **inside the animation tick** (§6) and only while both it and its
//! effect are active. §17 tests 6, 7 and 8 pin the action order, the
//! move-to-a-new-cell rule and the pause record.

use super::shipped::{game_flags, GameRecord};

/// The width of one city cell in metres (city slot 91).
pub const CELL_WIDTH: f32 = 16.0;

/// The cause byte a burn action sets (§9.3 step 1).
pub const FIRE_CAUSE: u32 = 4;

/// The spill a toxic action makes, whatever the record's amount (§9.3 quirk 6).
pub const TOXIC_SPILL: u32 = 120;

/// One action of a game effect's `Apply` (§9.3), in the order it runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameAction {
    /// §9.3 step 1: fire manager `IgniteCellRadius(cx, cz, 0, burn)`.
    Burn { x: i32, z: i32, radius: u32 },
    /// §9.3 step 1: the fire manager's `SetCause(4)`, right after the burn.
    SetFireCause { cause: u32 },
    /// §9.3 step 2: the toxic disaster's `AttemptDisasterStart(cx, cz)`. The
    /// record's amount is ignored (quirk 6).
    Toxic { x: i32, z: i32 },
    /// §9.3 step 3: `DemolishRegion(true, region, 0, flags, dezone, …)`.
    Demolish {
        x0: i32,
        z0: i32,
        x1: i32,
        z1: i32,
        flags: u32,
        dezone: bool,
    },
    /// §9.3 step 4: `ExtinguishCell(cx, cz, amount)`.
    Extinguish { x: i32, z: i32, amount: u32 },
    /// §9.3 step 5: a **queued** standard message.
    Message {
        msg_type: u32,
        data1: u64,
        data2: u32,
        data3: u32,
    },
    /// §9.3 step 6: the effect map (render). Reported so the order is
    /// complete; the map's shape is out of scope.
    Map { x: i32, z: i32 },
}

/// The world position of a cell's centre is not needed here; the game effect
/// works in cells. This is the §9.2 `SetTransform` rule: `cell :=
/// (floor(x / 16), floor(z / 16))`.
pub fn cell_of(x: f32, z: f32) -> (i32, i32) {
    ((x / CELL_WIDTH).floor() as i32, (z / CELL_WIDTH).floor() as i32)
}

/// §9.3 step 3: the demolish rectangle `[cx − r, cx + r] × [cz − r, cz + r]`
/// clipped to the city (city slots 93 and 94).
pub fn demolish_rect(cx: i32, cz: i32, radius: u32, cells: (u32, u32)) -> (i32, i32, i32, i32) {
    let r = radius as i32;
    let (nx, nz) = (cells.0 as i32, cells.1 as i32);
    let x0 = (cx - r).max(0);
    let z0 = (cz - r).max(0);
    let x1 = (cx + r).min(nx - 1);
    let z1 = (cz + r).min(nz - 1);
    (x0, z0, x1, z1)
}

/// The game effect's runtime state (§9.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameEffect {
    /// The record this component was built from.
    pub record: GameRecord,
    /// `pending`, initially 1: the next update applies.
    pub pending: bool,
    /// `started`, what `IsActive` returns.
    pub started: bool,
    /// `timer`, the elapsed effect-seconds.
    pub timer: f32,
    /// `count`, the number of applies so far.
    pub count: u32,
    /// The cell, initially (−1, −1).
    pub cell: (i32, i32),
    /// `+0x58`: the message's `data1`. Initially the description's message
    /// data; parameter 13 replaces it (§9.2).
    pub data: u64,
}

impl GameEffect {
    /// A game component built from its record. `data` starts as the record's
    /// message data (§9.2).
    pub fn new(record: GameRecord) -> GameEffect {
        GameEffect {
            record,
            pending: true,
            started: false,
            timer: 0.0,
            count: 0,
            cell: (-1, -1),
            data: 0,
        }
    }

    /// §9.2 `SetParameter(13, p, …)`: `data := *p`. Every other index is
    /// ignored. Returns whether it was accepted.
    pub fn set_parameter(&mut self, index: u32, value: u64) -> bool {
        if index == 13 {
            self.data = value;
            true
        } else {
            false
        }
    }

    /// §9.2 `SetTransform`: a change of cell sets `pending := 1`.
    pub fn set_position(&mut self, x: f32, z: f32) {
        let c = cell_of(x, z);
        if c != self.cell {
            self.cell = c;
            self.pending = true;
        }
    }

    /// The pause flags a `Start` raises (§9.2 step 2), in the order it raises
    /// them. `Pause`, `HiddenPause`, and the 24-hour clock with 1.
    pub fn start_pauses(&self) -> Vec<PauseAction> {
        let mut v = Vec::new();
        if self.record.flags & game_flags::PAUSE_SIM != 0 {
            v.push(PauseAction::Pause);
        }
        if self.record.flags & game_flags::PAUSE_SIM_HIDDEN != 0 {
            v.push(PauseAction::HiddenPause);
        }
        if self.record.flags & game_flags::PAUSE_CLOCK != 0 {
            v.push(PauseAction::Clock { on: true });
        }
        v
    }

    /// §9.2 `Stop` step 2: the pauses are undone in the order resume, hidden
    /// resume, clock with 0.
    pub fn stop_pauses(&self) -> Vec<PauseAction> {
        let mut v = Vec::new();
        if self.record.flags & game_flags::PAUSE_SIM != 0 {
            v.push(PauseAction::Resume);
        }
        if self.record.flags & game_flags::PAUSE_SIM_HIDDEN != 0 {
            v.push(PauseAction::HiddenResume);
        }
        if self.record.flags & game_flags::PAUSE_CLOCK != 0 {
            v.push(PauseAction::Clock { on: false });
        }
        v
    }

    /// §9.2 `Start`. Only when not already started: `started := 1`,
    /// `count := 0`, the pauses of [`GameEffect::start_pauses`], `timer := 0`.
    /// Returns whether it started.
    pub fn start(&mut self) -> bool {
        if self.started {
            return false;
        }
        self.started = true;
        self.count = 0;
        self.timer = 0.0;
        true
    }

    /// §9.2 `IsActive`: the `started` byte.
    pub fn is_active(&self) -> bool {
        self.started
    }

    /// §9.2 `Stop`. Only when started: `started := 0`, the pauses of
    /// [`GameEffect::stop_pauses`]. Returns whether it stopped.
    ///
    /// *[quirk]* §9.2 quirk 5: a `single` record goes inactive inside
    /// `Update` **without** calling `Stop`, so its pauses are never undone. No
    /// shipped record combines `single` with a pause.
    pub fn stop(&mut self) -> bool {
        if !self.started {
            return false;
        }
        self.started = false;
        true
    }

    /// §9.2 `Update(dt)`. Returns the actions to perform and the pauses this
    /// update undid. `cells` is the city's size (city slots 93 and 94).
    pub fn update(&mut self, dt: f32, cells: (u32, u32)) -> GameUpdate {
        if !self.started {
            return GameUpdate::default();
        }
        // single: Apply once, then go inactive without Stop (quirk 5).
        if self.record.flags & game_flags::SINGLE != 0 {
            let actions = self.apply(cells);
            self.started = false;
            return GameUpdate {
                actions,
                undo_pauses: Vec::new(),
            };
        }
        // 1. timer += dt, at 24 bits.
        self.timer = crate::x87::r24(f64::from(dt) + f64::from(self.timer)) as f32;
        // 2. duration expired: Stop(0) and return.
        if self.record.duration > 0.0 && self.timer > self.record.duration {
            self.started = false;
            return GameUpdate {
                actions: Vec::new(),
                undo_pauses: self.stop_pauses(),
            };
        }
        // 3. else, if pending: Apply, count += 1, pending := 0.
        if self.pending {
            let actions = self.apply(cells);
            self.count += 1;
            self.pending = false;
            return GameUpdate {
                actions,
                undo_pauses: Vec::new(),
            };
        }
        GameUpdate::default()
    }

    /// §9.3 `Apply`: the actions in their fixed order at the current cell.
    /// `cells` is the city's size (city slots 93 and 94).
    pub fn apply(&self, cells: (u32, u32)) -> Vec<GameAction> {
        let (cx, cz) = self.cell;
        let mut v = Vec::new();

        // 1. Burn, then the fire cause.
        if self.record.burn_radius > 0 {
            v.push(GameAction::Burn {
                x: cx,
                z: cz,
                radius: self.record.burn_radius,
            });
            v.push(GameAction::SetFireCause { cause: FIRE_CAUSE });
        }
        // 2. Toxic. The amount is ignored (quirk 6).
        if self.record.toxic_amount > 0 {
            v.push(GameAction::Toxic { x: cx, z: cz });
        }
        // 3. Demolish: the cell rectangle clipped to the city (§9.3 step 3).
        if self.record.demolish_radius > 0 {
            let (x0, z0, x1, z1) = demolish_rect(cx, cz, self.record.demolish_radius, cells);
            v.push(GameAction::Demolish {
                x0,
                z0,
                x1,
                z1,
                flags: self.record.demolish_flags,
                dezone: self.record.flags & game_flags::DEZONE != 0,
            });
        }
        // 4. Extinguish.
        if self.record.extinguish_amount > 0 {
            v.push(GameAction::Extinguish {
                x: cx,
                z: cz,
                amount: self.record.extinguish_amount,
            });
        }
        // 5. Message. data1 is the component's `data` field.
        if self.record.message_type != 0 {
            v.push(GameAction::Message {
                msg_type: self.record.message_type,
                data1: self.data,
                data2: cx as u32,
                data3: cz as u32,
            });
        }
        // 6. The effect map (render).
        if self.record.has_map {
            v.push(GameAction::Map { x: cx, z: cz });
        }
        v
    }
}

/// What one `Update` did (§9.2).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GameUpdate {
    /// The actions of `Apply`, in order (§9.3). Empty when nothing applied.
    pub actions: Vec<GameAction>,
    /// The pauses to undo because this update stopped the component itself.
    /// §9.2 step 2 (`Stop(0)` on expiry) undoes them; the `single` path does
    /// not (quirk 5), so this is empty there.
    pub undo_pauses: Vec<PauseAction>,
}

/// A pause a game effect raises or lifts (§9.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseAction {
    /// Simulator slot 9 `Pause`.
    Pause,
    /// Simulator slot 12 `Resume`.
    Resume,
    /// Simulator slot 10 `HiddenPause`.
    HiddenPause,
    /// Simulator slot 13 `HiddenResume`.
    HiddenResume,
    /// The 24-hour clock (`0x00B43DBC`) slot 26 with 1 or 0.
    Clock { on: bool },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::shipped::GAME_RECORDS;

    /// A 64 x 64 city (§17 test 6).
    const CITY: (u32, u32) = (64, 64);

    /// §17 test 7: a non-single game record applies on its first update. Moved
    /// within the same cell it does nothing; moved to the next cell it applies
    /// again (`count = 2`).
    #[test]
    fn a_moving_game_effect_applies_per_cell() {
        let mut g = GameEffect::new(GAME_RECORDS[29]); // burn 2, no pause
        g.set_position(16.0, 16.0);
        assert_eq!(g.cell, (1, 1));
        assert!(g.start());
        assert_eq!(g.count, 0);

        let a = g.update(0.1, CITY);
        assert_eq!(g.count, 1, "applies on its first update");
        assert!(a
            .actions
            .iter()
            .any(|x| matches!(x, GameAction::Burn { radius: 2, .. })));

        // Still in the same cell: nothing.
        g.set_position(17.0, 17.0);
        assert_eq!(g.cell, (1, 1));
        assert!(g.update(0.1, CITY).actions.is_empty());
        assert_eq!(g.count, 1, "the same cell does not re-apply");

        // The next cell: applies again.
        g.set_position(32.0, 16.0);
        assert_eq!(g.cell, (2, 1));
        assert!(!g.update(0.1, CITY).actions.is_empty());
        assert_eq!(g.count, 2);
    }

    /// §17 test 6: game record 17 at cell (40, 40) in a 64 × 64 city runs
    /// `IgniteCellRadius(40, 40, 0, 14)`, `SetCause(4)`, then `DemolishRegion`
    /// over [30, 50] × [30, 50] with flags 2 and dezone false, and no message.
    #[test]
    fn game_record_17_action_order() {
        let mut g = GameEffect::new(GAME_RECORDS[17]);
        g.set_position(40.0 * 16.0, 40.0 * 16.0);
        assert_eq!(g.cell, (40, 40));
        assert!(g.start());

        let a = g.update(0.1, CITY);
        assert_eq!(
            a.actions,
            vec![
                GameAction::Burn {
                    x: 40,
                    z: 40,
                    radius: 14
                },
                GameAction::SetFireCause { cause: 4 },
                GameAction::Demolish {
                    x0: 30,
                    z0: 30,
                    x1: 50,
                    z1: 50,
                    flags: 2,
                    dezone: false
                },
            ],
            "burn, cause, demolish, no message"
        );
    }

    /// §17 test 6 tail: at cell (2, 60) the rectangle is clipped to
    /// [0, 12] × [50, 63] in a 64 × 64 city.
    #[test]
    fn the_demolish_rectangle_is_clipped_to_the_city() {
        assert_eq!(demolish_rect(40, 40, 10, (64, 64)), (30, 30, 50, 50));
        assert_eq!(demolish_rect(2, 60, 10, (64, 64)), (0, 50, 12, 63));
    }

    /// §17 test 8: game record 3, a hidden pause of 5 s.
    #[test]
    fn a_pause_record_pauses_and_resumes_once() {
        let mut g = GameEffect::new(GAME_RECORDS[3]);
        assert_eq!(g.record.flags, game_flags::PAUSE_SIM_HIDDEN);
        assert_eq!(g.record.duration, 5.0);
        assert!(g.start(), "Start");
        assert_eq!(g.start_pauses(), vec![PauseAction::HiddenPause], "HiddenPause once");
        assert_eq!(g.stop_pauses(), vec![PauseAction::HiddenResume]);

        for i in 1..=5 {
            let u = g.update(1.0, CITY);
            assert!(u.undo_pauses.is_empty(), "nothing to undo yet");
            assert!(g.is_active(), "still running after update {i}, timer = {}", g.timer);
        }
        assert_eq!(g.timer, 5.0);
        let u = g.update(1.0, CITY);
        assert_eq!(g.timer, 6.0);
        assert!(!g.is_active(), "at timer = 6 > 5 it stops");
        assert_eq!(
            u.undo_pauses,
            vec![PauseAction::HiddenResume],
            "Stop(0) undoes the pause"
        );

        assert!(!g.stop(), "a second Stop does nothing");
    }

    /// §9.2 quirk 5: a `single` record applies once and goes inactive without
    /// `Stop`, so its pauses are never undone.
    #[test]
    fn a_single_record_applies_once_and_leaves_its_pauses() {
        let mut g = GameEffect::new(GAME_RECORDS[1]);
        assert!(g.record.flags & game_flags::SINGLE != 0);
        g.set_position(0.0, 0.0);
        assert!(g.start());

        let a = g.update(0.1, CITY);
        assert!(
            a.actions.iter().any(|x| matches!(
                x,
                GameAction::Demolish {
                    x0: 0,
                    z0: 0,
                    x1: 10,
                    z1: 10,
                    ..
                }
            )),
            "the rect is clipped to the city"
        );
        assert!(
            a.undo_pauses.is_empty(),
            "quirk 5: the single path never undoes its pauses"
        );
        assert!(!g.is_active(), "started := 0 inside Update");
        assert_eq!(g.count, 0, "the single path does not count its apply");
        assert!(g.update(0.1, CITY).actions.is_empty(), "it never applies again");
    }

    /// §9.2 `SetParameter(13)`: `data` becomes the value and is what a
    /// location message carries as `data1` (§10.2). Other indices are ignored.
    #[test]
    fn parameter_13_replaces_the_message_data() {
        let mut g = GameEffect::new(GAME_RECORDS[19]); // message 0x2A7B582E
        assert!(g.set_parameter(13, 0x1111));
        assert_eq!(g.data, 0x1111);
        assert!(!g.set_parameter(7, 5), "only index 13 is accepted");
        assert_eq!(g.data, 0x1111);

        g.set_position(3.0 * 16.0, 4.0 * 16.0);
        assert!(g.start());
        let a = g.update(0.1, CITY);
        assert_eq!(
            a.actions,
            vec![GameAction::Message {
                msg_type: 0x2A7B_582E,
                data1: 0x1111,
                data2: 3,
                data3: 4,
            }]
        );
    }

    /// §9.3 step 5: the toxic amount is ignored, the spill is always 120.
    #[test]
    fn toxic_carries_no_amount() {
        let g = GameEffect::new(GAME_RECORDS[55]);
        assert_eq!(g.record.toxic_amount, 20, "the record says 20");
        let mut g = g;
        g.set_position(0.0, 0.0);
        assert!(g.start());
        let a = g.update(0.1, CITY);
        assert_eq!(
            a.actions,
            vec![GameAction::Toxic { x: 0, z: 0 }],
            "the amount is not in the action"
        );
        assert_eq!(TOXIC_SPILL, 120, "the spill is always 120");
    }

    /// §9.2 `Start` / `Stop` only act when the state changes.
    #[test]
    fn start_and_stop_are_once_only() {
        let mut g = GameEffect::new(GAME_RECORDS[0]);
        assert!(g.start());
        assert!(!g.start(), "a second Start does nothing");
        assert!(g.stop());
        assert!(!g.stop());
    }
}
