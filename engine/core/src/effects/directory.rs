// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The effect directory as the manager sees it (`docs/specs/effects.md` §3,
//! §4.1, §5.2).
//!
//! The directory is the one resource the manager reads descriptions from: type
//! `0xEA5118B0`, group `0xEA5118B1`, instance 1 of `SimCity_1.dat`, holding
//! **1154** named visual effects made of **3373** components (§3). The
//! on-disk shapes live in `fc4_formats::effects`; this module is the
//! lookup half — the case-insensitive name map and the priority filter of
//! §4.1 — over records the caller has already parsed.

use super::component_type;

/// One component of a description, as the builder needs it (§3, §5.2, §5.3).
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentDesc {
    /// `+0x00`: the child effect's name for a meta component, the asset name
    /// for particles and decals, empty otherwise.
    pub name: Vec<u8>,
    /// `+0x0C`: [`component_type`].
    pub kind: u8,
    /// `+0x10`, masked to `& 3`: bit 0 is ignored by the finish test (§7).
    pub flags: u32,
    /// `+0x4C`, `+0x4D`: the level-of-detail range `[min, max)` (§5.2).
    pub lod_min: u8,
    pub lod_max: u8,
    /// `+0x64`: 0 = always created; `g > 0` is a select group (§5.3).
    pub select_group: u16,
    /// `+0x66`.
    pub select_weight: u16,
    /// `+0x68`: an index into the description table of the component's type.
    pub index: u32,
}

impl ComponentDesc {
    /// §5.2: whether this component exists at the current level of detail `L`.
    pub fn exists_at(&self, lod: u8) -> bool {
        self.lod_min <= lod && lod < self.lod_max
    }

    /// §7: whether the finish test skips this component. It is the **runtime**
    /// bit 2 that is tested, which is this description bit copied across
    /// (`super::runtime_flags`).
    pub fn ignored_by_finish(&self) -> bool {
        super::runtime_flags(self.flags, self.kind) & super::flags::IGNORED != 0
    }

    /// Whether this component reaches a brush or game record (§5.2).
    ///
    /// §5.2 Corrects: "every component of every shipped effect has the range
    /// `[1, 6)`" is **false** — 2533 of 3373 do and 840 do not. The narrower
    /// claim the simulation depends on, and the one worth asserting, is that
    /// every component that reaches a brush or game record — directly or
    /// through a meta component — is `[1, 6)`.
    pub fn reaches_table(&self) -> bool {
        self.lod_min == 1 && self.lod_max == 6
    }

    /// Whether this component is a meta component (a nested visual effect).
    pub fn is_meta(&self) -> bool {
        self.kind == component_type::META
    }

    /// Whether this component indexes a brush or game record.
    pub fn is_brush_or_game(&self) -> bool {
        self.kind == component_type::BRUSH || self.kind == component_type::GAME
    }
}

/// One visual-effect description (§3).
#[derive(Clone, Debug, PartialEq)]
pub struct EffectDesc {
    /// `+0x00`, masked to 9 bits. Bit 1 (2) is "no auto stop"; bit 7 (0x80)
    /// finishes only after a soft stop (§11.1).
    pub flags: u32,
    /// `+0x04`: 0 is never chosen (§4.1).
    pub priority: u32,
    /// `+0x08`.
    pub components: Vec<ComponentDesc>,
    /// `+0x2C`.
    pub name: Vec<u8>,
}

/// An entry of the name map (§3 step 8): a name and the description index it
/// points at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NameEntry {
    pub name: Vec<u8>,
    pub description_index: u32,
}

/// The effect directory as the manager uses it (§3, §4.1).
#[derive(Clone, Debug, Default)]
pub struct EffectDirectory {
    descriptions: Vec<EffectDesc>,
    /// The name map, **in file order**. §4.1 step 3.2: when several
    /// descriptions share the highest qualifying priority, the first one met in
    /// the name map wins.
    names: Vec<NameEntry>,
}

/// The result of §4.1 step 3, `FindEffectDesc`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Found {
    /// A description was chosen.
    Index(i32),
    /// The name exists but no description has a priority in `(0, maxPriority]`.
    /// §4.1: the call **still succeeds** and the effect gets index −1.
    BelowPriority,
    /// No description has this name: §4.1 step 2 returns false and leaves the
    /// caller's out-parameter alone.
    NoSuchName,
}

impl EffectDirectory {
    /// An empty directory.
    pub fn new() -> EffectDirectory {
        EffectDirectory::default()
    }

    /// Builds a directory from its parsed descriptions and name map.
    pub fn from_parts(descriptions: Vec<EffectDesc>, names: Vec<NameEntry>) -> EffectDirectory {
        EffectDirectory { descriptions, names }
    }

    /// The descriptions, in section order.
    pub fn descriptions(&self) -> &[EffectDesc] {
        &self.descriptions
    }

    /// The name map, in file order.
    pub fn names(&self) -> &[NameEntry] {
        &self.names
    }

    /// One description by index.
    pub fn description(&self, index: i32) -> Option<&EffectDesc> {
        if index < 0 {
            return None;
        }
        self.descriptions.get(index as usize)
    }

    /// §3 step 8: names are looked up **case-insensitively**. The lookup key is
    /// lowercased by §4.1 step 2 before it gets here, so this compares
    /// ASCII-lowercased bytes.
    pub fn name_matches(&self, entry: &NameEntry, lower_name: &[u8]) -> bool {
        entry.name.len() == lower_name.len()
            && entry
                .name
                .iter()
                .zip(lower_name)
                .all(|(a, b)| a.to_ascii_lowercase() == *b)
    }

    /// §4.1 step 3, `FindEffectDesc`.
    ///
    /// 1. Among the descriptions with this name, take the one with the highest
    ///    priority `p` such that `0 < p ≤ maxPriority`.
    /// 2. If several have that highest priority, the first one met in the name
    ///    map wins.
    /// 3. If none qualifies, the index is −1 and the call **still succeeds**.
    pub fn find(&self, lower_name: &[u8], max_priority: u32) -> Found {
        let mut saw_name = false;
        let mut best: Option<(u32, i32)> = None;
        for entry in &self.names {
            if !self.name_matches(entry, lower_name) {
                continue;
            }
            saw_name = true;
            let index = entry.description_index as i32;
            let Some(d) = self.description(index) else {
                continue;
            };
            let p = d.priority;
            if p == 0 || p > max_priority {
                continue;
            }
            // Strictly greater, so the first met in the name map wins a tie.
            if best.map_or(true, |(bp, _)| p > bp) {
                best = Some((p, index));
            }
        }
        match best {
            Some((_, index)) => Found::Index(index),
            None if saw_name => Found::BelowPriority,
            None => Found::NoSuchName,
        }
    }

    /// `HasEffectDesc` (§4.2): whether an ID pair names a description. The
    /// directory keys by name; an ID pair is looked up by the name map's
    /// description index standing in for the pair. *[choice]* the spec gives
    /// no ID-pair table, so this answers "the index exists".
    pub fn has_id_pair(&self, _id0: u32, _id1: u32, index: u32) -> bool {
        (index as usize) < self.descriptions.len()
    }
}

/// ASCII-lowercase a name, the §4.1 step 2 "the name is lowercased" rule.
pub fn lowercase(name: &[u8]) -> Vec<u8> {
    name.iter().map(u8::to_ascii_lowercase).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desc(name: &[u8], priority: u32, components: Vec<ComponentDesc>) -> EffectDesc {
        EffectDesc {
            flags: 0,
            priority,
            components,
            name: name.to_vec(),
        }
    }

    fn named(index: u32, name: &[u8]) -> NameEntry {
        NameEntry {
            name: name.to_vec(),
            description_index: index,
        }
    }

    fn component(kind: u8, lod_min: u8, lod_max: u8) -> ComponentDesc {
        ComponentDesc {
            name: Vec::new(),
            kind,
            flags: 0,
            lod_min,
            lod_max,
            select_group: 0,
            select_weight: 0,
            index: 0,
        }
    }

    /// §17 test 9: two descriptions named `x`, priorities 1 and 3. With max
    /// priority 2, `x` resolves to the priority-1 one. With max priority 0,
    /// `CreateVisualEffect("x")` returns true with index −1.
    #[test]
    fn the_priority_filter_picks_the_highest_qualifying_priority() {
        let dir = EffectDirectory::from_parts(
            vec![desc(b"x", 1, vec![]), desc(b"x", 3, vec![])],
            vec![named(0, b"x"), named(1, b"x")],
        );
        assert_eq!(dir.find(b"x", 2), Found::Index(0), "priority 1 is the highest <= 2");
        assert_eq!(dir.find(b"x", 3), Found::Index(1), "priority 3 qualifies at 3");
        assert_eq!(dir.find(b"x", 5), Found::Index(1), "and at the construction default 5");
        assert_eq!(dir.find(b"x", 0), Found::BelowPriority, "0 < p is never satisfied");
    }

    /// §4.1 step 3.2: a tie is broken by the first entry met in the name map.
    #[test]
    fn a_tie_is_broken_by_name_map_order() {
        let dir = EffectDirectory::from_parts(
            vec![desc(b"x", 3, vec![]), desc(b"x", 3, vec![])],
            // The name map lists index 1 first.
            vec![named(1, b"x"), named(0, b"x")],
        );
        assert_eq!(dir.find(b"x", 5), Found::Index(1));
    }

    /// §4.1 step 2: a name no description carries fails and leaves the
    /// caller's out-parameter alone.
    #[test]
    fn an_unknown_name_is_not_found() {
        let dir = EffectDirectory::from_parts(vec![desc(b"x", 1, vec![])], vec![named(0, b"x")]);
        assert_eq!(dir.find(b"y", 5), Found::NoSuchName);
    }

    /// §3 step 8: names are looked up case-insensitively.
    #[test]
    fn names_match_case_insensitively() {
        let dir = EffectDirectory::from_parts(
            vec![desc(b"Volcano_Control", 1, vec![])],
            vec![named(0, b"Volcano_Control")],
        );
        let lower = lowercase(b"VOLCANO_control");
        assert_eq!(lower, b"volcano_control");
        assert_eq!(dir.find(&lower, 5), Found::Index(0));
    }

    /// §5.2 Corrects, conformance test 14: the shipped directory has 3373
    /// components of which 2533 are `[1, 6)` and 840 are not, so components
    /// **do** come and go with the zoom. The narrower claim is that everything
    /// reaching a brush or game record is `[1, 6)`.
    #[test]
    fn lod_ranges_are_not_uniform_and_the_narrow_claim_is_the_one_to_assert() {
        // The 840 that are not [1, 6): the measured distribution of §5.2.
        let not_full = [
            (5u8, 6u8, 182u32),
            (4, 5, 156),
            (3, 4, 141),
            (2, 3, 106),
            (1, 2, 101),
            (4, 6, 98),
            (1, 3, 30),
            (3, 6, 27),
            (1, 4, 23),
            (2, 6, 10),
            (3, 5, 8),
            (1, 5, 5),
        ];
        // §5.2 Corrects: the text says 2533 of 3373 are [1, 6) and 840 are
        // not. The per-range table it prints alongside **sums to 887**, not
        // 840, so the two cannot both be right. The text figures are the ones
        // the correction rests on and the ones asserted here; the table's own
        // sum is pinned too so the discrepancy stays visible. See the lane
        // report's question on §5.2.
        let total: u32 = not_full.iter().map(|(_, _, n)| n).sum();
        assert_eq!(total, 887, "the per-range table of §5.2 sums to 887");
        assert_eq!(3373 - 2533, 840, "the text's 840 that are not [1, 6)");
        assert_eq!(2533 + 840, 3373, "the text's 2533 that are");
        assert_ne!(total, 840, "the table and the text disagree by 47");

        for (min, max, count) in not_full {
            let c = component(component_type::PARTICLES, min, max);
            assert!(!c.reaches_table(), "{count} components have [{min}, {max})");
            assert!(
                !c.exists_at(1) || !c.exists_at(5) || min == 5,
                "[{min}, {max}) varies with zoom"
            );
        }
        // The claim the simulation depends on.
        for kind in [component_type::BRUSH, component_type::GAME] {
            let c = component(kind, 1, 6);
            assert!(c.reaches_table(), "a {kind} component reaches its record");
            assert!(c.exists_at(1) && c.exists_at(5));
        }
    }

    /// §7: the description bit is the ignore bit, and it is what gets copied to
    /// the runtime record.
    #[test]
    fn the_ignore_bit_is_the_description_bit() {
        let counting = component(component_type::PARTICLES, 1, 6);
        assert!(!counting.ignored_by_finish());
        let mut ignored = component(component_type::GAME, 1, 6);
        ignored.flags = 1;
        assert!(ignored.ignored_by_finish(), "game components always carry it");
    }
}
