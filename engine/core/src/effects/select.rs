// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The random "select" groups of `docs/specs/effects.md` §5.3.
//!
//! Components of the same select group `g > 0` are alternatives: exactly one of
//! them is built. The manager draws from its own `cRZRandomSuper` at `+0x1C0`,
//! seeded with the constant **`0x1105`** at construction (`0x005912E9`–
//! `0x0059133F`). That generator is never reseeded and is used only here
//! (§2, §14).

use crate::rng::{FixedSeeds, RzRandomSuper};

/// The seed the manager's select generator is built with (§2). A constant, so a
/// fresh city's first volcano picks a predictable creator (§14).
pub const SELECT_SEED: u32 = 0x1105;

/// The top of the draw's range: `RandomUint32Uniform(0x10000)` returns a value
/// in `[0, 0xFFFF]`, and §5.3 sets `r` to this after a member is chosen so the
/// rest of the group is skipped.
pub const SELECT_RANGE: u32 = 0x10000;

/// One component's select-group placement (§3, `+0x64` group and `+0x66`
/// weight).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SelectPlacement {
    /// `+0x64`: 0 means "always created"; `g > 0` is a select group.
    pub group: u16,
    /// `+0x66`.
    pub weight: u16,
}

impl SelectPlacement {
    /// A component that is always created (group 0).
    pub const ALWAYS: SelectPlacement = SelectPlacement { group: 0, weight: 0 };

    /// A member of select group `group` with the given weight.
    pub const fn member(group: u16, weight: u16) -> SelectPlacement {
        SelectPlacement { group, weight }
    }

    /// Whether this component takes part in a select draw (§5.3).
    pub fn is_select(&self) -> bool {
        self.group > 0
    }
}

/// §5.3: which of `placements` the manager creates, walking the list in order.
///
/// 1. At the **first** component of a group — a group number different from the
///    previous component's — draw `r := RandomUint32Uniform(0x10000)` and reset
///    the running sum to 0.
/// 2. Each member adds its weight to the running sum; the **first member whose
///    running sum is ≥ r** is created.
/// 3. After that `r := 0x10000`, so the remaining members are skipped.
///
/// Group 0 is "always created" and draws nothing.
///
/// With weights summing to 65535 and `r` in `[0, 65535]`, exactly one member of
/// each group is created. A group whose weights sum to less than `r` creates
/// none; that is the literal reading of "the first member whose running sum is
/// ≥ r", and it is why the shipped weights sum to 65535.
pub fn select_components<F>(placements: &[SelectPlacement], mut draw: F) -> Vec<bool>
where
    F: FnMut() -> u32,
{
    let mut created = vec![false; placements.len()];
    let mut previous_group: Option<u16> = None;
    let mut r: u32 = SELECT_RANGE;
    let mut sum: u32 = 0;

    for (i, p) in placements.iter().enumerate() {
        if !p.is_select() {
            created[i] = true;
            previous_group = Some(p.group);
            continue;
        }
        // A new run of a group starts a fresh draw.
        if previous_group != Some(p.group) {
            r = draw();
            sum = 0;
        }
        previous_group = Some(p.group);
        sum = sum.saturating_add(u32::from(p.weight));
        if sum >= r {
            created[i] = true;
            // Skip the rest of this run.
            r = SELECT_RANGE;
        }
    }
    created
}

/// The manager's select generator, built the way §2 builds it: seeded with
/// [`SELECT_SEED`] at construction and never reseeded.
///
/// The seed is a constant, so the clock is never consulted (`0x1105` is not
/// [`CLOCK_SEED`](crate::rng::CLOCK_SEED)); the zeros stand for "unused".
pub fn select_generator() -> RzRandomSuper {
    RzRandomSuper::new(
        SELECT_SEED,
        &FixedSeeds {
            wall_clock: 0,
            process_clock: 0,
        },
    )
}

/// `RandomUint32Uniform(0x10000)` from a select generator (§5.3).
pub fn draw_select(generator: &mut RzRandomSuper) -> u32 {
    generator.uniform_u32(SELECT_RANGE)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §17 test 3: a fresh manager's first tempered word is `0xAAE64AC3`, so
    /// `r = 43750`, and the `volcano_control` group's running sums 7281, 14562,
    /// … first reach ≥ 43750 at the 7th member — `volcanocreator4a`, brush
    /// record 8.
    #[test]
    fn the_first_select_draw_picks_volcanocreator4a() {
        let mut g = select_generator();
        assert_eq!(g.next_u32(), 0xAAE6_4AC3, "the first tempered word (§17 test 3)");
        let mut g = select_generator();
        assert_eq!(draw_select(&mut g), 43750, "0xAAE6_4AC3 >> 16 = 0xAAE6");

        // volcano_control's nine meta creators, weights 7281 x 8 and 7287.
        let weights = [7281u16, 7281, 7281, 7281, 7281, 7281, 7281, 7281, 7287];
        let placements: Vec<SelectPlacement> = weights.iter().map(|&w| SelectPlacement::member(1, w)).collect();
        let created = select_components(&placements, || 43750);
        assert_eq!(
            created.iter().filter(|&&c| c).count(),
            1,
            "exactly one creator is built"
        );
        assert!(created[6], "the 7th member is volcanocreator4a");
    }

    /// §17 test 3: the next five draws give `r` = 24188, 47502, 45072, 37594
    /// and 53135, picking members 4, 7, 7, 6 and 8 (1-based).
    #[test]
    fn the_next_five_draws_pick_the_measured_members() {
        let weights = [7281u16, 7281, 7281, 7281, 7281, 7281, 7281, 7281, 7287];
        let placements: Vec<SelectPlacement> = weights.iter().map(|&w| SelectPlacement::member(1, w)).collect();
        for (r, one_based) in [(24188u32, 4usize), (47502, 7), (45072, 7), (37594, 6), (53135, 8)] {
            let created = select_components(&placements, || r);
            let chosen: Vec<usize> = created
                .iter()
                .enumerate()
                .filter(|(_, &c)| c)
                .map(|(i, _)| i + 1)
                .collect();
            assert_eq!(chosen, vec![one_based], "r = {r} picks member {one_based}");
        }
    }

    /// §5.3: the draw happens once per group run, the first member whose
    /// running sum reaches `r` wins, and `r` is then pinned so the rest are
    /// skipped. Group 0 is always created and draws nothing.
    #[test]
    fn the_walk_draws_once_per_group_and_skips_the_rest() {
        // Two groups and a group-0 component between them. The group-0 entry
        // ends the first run, so the second group draws again.
        let p = [
            SelectPlacement::member(1, 100),
            SelectPlacement::member(1, 100),
            SelectPlacement::ALWAYS,
            SelectPlacement::member(2, 100),
            SelectPlacement::member(2, 100),
        ];
        let mut draws = 0u32;
        let created = select_components(&p, || {
            draws += 1;
            // First draw 150 -> the second member of group 1 (sum 200 >= 150).
            // Second draw 50 -> the first member of group 2 (sum 100 >= 50).
            if draws == 1 {
                150
            } else {
                50
            }
        });
        assert_eq!(draws, 2, "one draw per group run");
        assert_eq!(created, vec![false, true, true, true, false]);
    }

    /// §5.3: a zero-weight member still counts, because the test is on the
    /// running sum after its own weight is added. `r = 0` therefore picks the
    /// **first** member (`sum ≥ 0` always holds there), which is what "r in
    /// [0, 65535] creates exactly one member" means at the low end.
    #[test]
    fn zero_weights_and_zero_draws() {
        let p = [
            SelectPlacement::member(1, 0),
            SelectPlacement::member(1, 0),
            SelectPlacement::member(1, 5),
            SelectPlacement::member(1, 5),
        ];
        let created = select_components(&p, || 0);
        assert_eq!(created, vec![true, false, false, false], "r = 0 takes the first");

        // Weights that never reach r create nothing.
        let created = select_components(&p, || 100);
        assert_eq!(created, vec![false, false, false, false]);
    }

    /// §5.3: every group 0 component is created, whatever the draw.
    #[test]
    fn group_zero_is_always_created() {
        let p = [SelectPlacement::ALWAYS, SelectPlacement::ALWAYS];
        assert_eq!(
            select_components(&p, || -> u32 { unreachable!("group 0 draws nothing") }),
            vec![true, true]
        );
    }
}
