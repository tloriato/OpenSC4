// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version. This program is distributed WITHOUT ANY WARRANTY; see
// the GNU General Public License for more details. You should have received a
// copy of the license along with this program (see LICENSE at the repository
// root); if not, see <https://www.gnu.org/licenses/>.

//! `cSC4LandValueSimulator` (`docs/specs/landvalue.md`), converted from `engine/core/src/landvalue.rs` one
//! function at a time. Section references `§n` point at that spec.

use crate::game;
use crate::port::Port;
use crate::x87::F80;

layout! {
    /// `cSC4LandValueSimulator`, offsets from its `cISC4LandValueSimulator` base L (`docs/abi/layouts.md`,
    /// "Land-value native progress"). The altitude fields are the ones `SetupAltitudeEffect` (0x006BD750) reads
    /// and writes (§7.3).
    pub struct LandValueSim {
        /// Shared signed reference count.
        0x10 => reference_count / set_reference_count: i32,
        /// Message server used to post cycle notifications.
        0x14 => message_server / set_message_server: *mut u8,
        /// The zone manager (`cISC4ZoneManager*`), taken in `Init`.
        0x20 => zone_manager / set_zone_manager: *mut u8,
        /// The intrinsic land value grid (`cISC4SimGrid<uint8>*`), the first grid `Init` creates (§2).
        0x24 => intrinsic_grid / set_intrinsic_grid: *mut u8,
        /// Total land value grid, queried in cell coordinates.
        0x28 => total_grid / set_total_grid: *mut u8,
        /// pivot (§7.3).
        0x60 => pivot / set_pivot: f32,
        /// lowRange (§7.3).
        0x64 => low_range / set_low_range: f32,
        /// highRange (§7.3).
        0x68 => high_range / set_high_range: f32,
        /// topIndex (§7.3).
        0x6C => top_index / set_top_index: i32,
        /// Average land value from the last completed pass.
        0x74 => average_land_value / set_average_land_value: u8,
        /// meanAlt (§3).
        0x78 => mean_altitude / set_mean_altitude: f32,
        /// Unsigned low/medium and medium/high wealth thresholds.
        0x7F => low_wealth_boundary / set_low_wealth_boundary: u8,
        0x80 => high_wealth_boundary / set_high_wealth_boundary: u8,
        /// The altitude curve (`cSC4ResponseCurve*`, tuning `0x47E2C320`).
        0x84 => altitude_curve / set_altitude_curve: *mut u8,
        /// minAlt (§3).
        0x98 => min_altitude / set_min_altitude: f32,
        /// maxAlt (§3).
        0x9C => max_altitude / set_max_altitude: f32,
        /// modeAlt (§3).
        0xA0 => mode_altitude / set_mode_altitude: f32,
    }
}

/// `cISC4SimGrid` slot 10 (+0x28) `GetTractSize()` and slot 11 (+0x2C) `GetTractShift()` (simgrid.md §4).
const GRID_TRACT_SIZE: usize = 0x28;
const GRID_TRACT_SHIFT: usize = 0x2C;
/// `cISC4ZoneManager` slot 12 (+0x30) `GetZoneType(x, z)` (§12).
const ZONE_TYPE: usize = 0x30;
/// The 50 of §7.3 (the game reads it from its read-only constant at 0x00A84D24).
const ALTITUDE_SCALE: f32 = 50.0;

port! {
    /// `TractIsZoned(tx, tz)` (§2 `IsZoned`): whether any cell of the tract, `[tx·S, tx·S + S) × [tz·S, tz·S + S)`,
    /// has a nonzero zone type (the full 32-bit value), scanning x outer and z inner and stopping at the first.
    ///
    /// Calls, in order: the intrinsic grid's `GetTractShift()` then `GetTractSize()` (the grid pointer read
    /// before each), then `GetZoneType(x, z)` per cell (the zone manager pointer read before each). Bounds wrap
    /// like 32-bit integers; the shift count is taken modulo 32.
    ///
    /// EAX: AL is the answer. The upper three bytes are what the original leaves there: those of the zone
    /// type that was found, else those of `tz·S + S`.
    #[port(address = 0x006B_D540, owner = "landvalue", name = "cSC4LandValueSimulator::TractIsZoned")]
    pub unsafe extern "thiscall" fn fc4p_006bd540(this: *mut u8, tx: i32, tz: i32) -> u32 {
        // SAFETY: the game calls this on its land value simulator.
        let lv = unsafe { LandValueSim::from_ptr(this) };
        // SAFETY (the three calls): the grid and the zone manager are the simulator's live objects.
        let shift = unsafe { vcall!(lv.intrinsic_grid(), GRID_TRACT_SHIFT => fn() -> u32) };
        let size = unsafe { vcall!(lv.intrinsic_grid(), GRID_TRACT_SIZE => fn() -> i32) };
        let (x0, z0) = (tx.wrapping_shl(shift), tz.wrapping_shl(shift));
        let (x1, z1) = (x0.wrapping_add(size), z0.wrapping_add(size));
        let mut x = x0;
        while x < x1 {
            let mut z = z0;
            while z < z1 {
                let zone = unsafe { vcall!(lv.zone_manager(), ZONE_TYPE => fn(i32, i32) -> u32, x, z) };
                if zone != 0 {
                    return (zone & !0xFF) | 1;
                }
                z += 1;
            }
            x += 1;
        }
        z1 as u32 & !0xFF
    }
}

port! {
    /// `LookupAltitudeEffect(a)` (§7.3 `AltitudeEffect`): 0 below minAlt; else the curve index
    /// `trunc((a − minAlt) / lowRange × 50)` below the pivot, `trunc((topIndex − 50) / highRange × (a − pivot) + 50)`
    /// from it on (NaN takes this branch), the altitude curve's output at that index (as a float), truncated.
    ///
    /// All arithmetic is x87 arithmetic in the original's order under the caller's control word: `topIndex − 50`
    /// is loaded as an exact integer, intermediates keep the register exponent range, and both truncations are
    /// the game's float-to-integer helper (64-bit truncation, low 32 bits kept). One call: the curve's
    /// `GetOutputValue` (0x005F4250), whose ST0 result is used unrounded.
    ///
    /// EAX: AL is the effect byte. Otherwise EAX is the helper's low 32 bits; below minAlt it is the caller's
    /// upper half, the comparison's status word in AH, and 0 in AL.
    #[port(address = 0x006B_D810, owner = "landvalue", name = "cSC4LandValueSimulator::LookupAltitudeEffect",
        entry_eax)]
    pub unsafe extern "thiscall" fn fc4p_006bd810(this: *mut u8, [entry_eax: u32], altitude: f32) -> u32 {
        // SAFETY: the game calls this on its land value simulator.
        let lv = unsafe { LandValueSim::from_ptr(this) };
        let a = F80::from_f32(altitude);
        let low = a.compare(F80::from_f32(lv.min_altitude()));
        if low.less() {
            return (entry_eax & 0xFFFF_0000) | u32::from(low.status_word() & 0xFF00);
        }
        let scale = F80::from_f32(ALTITUDE_SCALE);
        let position = if a.compare(F80::from_f32(lv.pivot())).less() {
            (a - F80::from_f32(lv.min_altitude())) / F80::from_f32(lv.low_range()) * scale
        } else {
            let slope = F80::from_i32(lv.top_index().wrapping_sub(50)) / F80::from_f32(lv.high_range());
            slope * (a - F80::from_f32(lv.pivot())) + scale
        };
        let index = F80::from_i32(position.ftol() as i32).to_f32();
        // SAFETY: the simulator's live altitude curve.
        let effect = unsafe { game::response_curve_output(lv.altitude_curve(), index) };
        effect.ftol() as u32
    }
}

// The rest of the class, one file per group of functions (each file has its own `PORTS`).
pub mod com;
pub mod grid;
pub mod idle;
pub mod io;
pub mod life;
pub mod terrain;

const TRACT: &[Port] = &[fc4p_006bd540::PORT, fc4p_006bd810::PORT];
const COUNT: usize = TRACT.len()
    + com::PORTS.len()
    + grid::PORTS.len()
    + idle::PORTS.len()
    + io::PORTS.len()
    + life::PORTS.len()
    + terrain::PORTS.len();
const ALL: [Port; COUNT] = crate::port::concat(&[
    TRACT,
    com::PORTS,
    grid::PORTS,
    idle::PORTS,
    io::PORTS,
    life::PORTS,
    terrain::PORTS,
]);

/// This module's replacements.
pub const PORTS: &[Port] = &ALL;

#[cfg(test)]
mod tests {
    //! Moved from the core's predicates and altitude effect (`engine/core/src/landvalue.rs`, the conformance
    //! vectors `lv16_08_09_altitude_effect` and the §2 predicate), run on objects at the original layout with
    //! fakes for the grid, the zone manager and the curve.

    use super::*;
    use crate::testing::{call_thiscall_with_eax, fake, GameFpu, Object};
    use std::cell::RefCell;

    #[derive(Clone, Copy, Debug, PartialEq)]
    enum Call {
        Shift(usize),
        Size(usize),
        Zone(usize, i32, i32),
        Curve(usize, f32),
    }

    thread_local! {
        static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
    }

    fn log(c: Call) {
        CALLS.with(|l| l.borrow_mut().push(c));
    }

    fn take_calls() -> Vec<Call> {
        CALLS.with(|l| std::mem::take(&mut *l.borrow_mut()))
    }

    /// A grid: vtable, tract shift, tract size, and optionally a grid to put at the simulator's +0x24 when its
    /// shift is read.
    #[repr(C)]
    struct FakeGrid {
        vtable: *const usize,
        shift: u32,
        size: i32,
        swap: Option<(*mut u8, *mut u8)>,
    }

    extern "thiscall" fn grid_shift(this: *mut FakeGrid) -> u32 {
        log(Call::Shift(this as usize));
        // SAFETY: the test's grid.
        let g = unsafe { &*this };
        if let Some((lv, next)) = g.swap {
            // SAFETY: the test's simulator object.
            unsafe { LandValueSim::from_ptr(lv) }.set_intrinsic_grid(next);
        }
        g.shift
    }

    extern "thiscall" fn grid_size(this: *mut FakeGrid) -> i32 {
        log(Call::Size(this as usize));
        // SAFETY: the test's grid.
        unsafe { (*this).size }
    }

    /// A zone manager: vtable, the zoned cells, and optionally a manager to put at the simulator's +0x20 after
    /// the first call.
    #[repr(C)]
    struct FakeZones {
        vtable: *const usize,
        zoned: Vec<(i32, i32, u32)>,
        swap: Option<(*mut u8, *mut u8)>,
    }

    extern "thiscall" fn zone_type(this: *mut FakeZones, x: i32, z: i32) -> u32 {
        log(Call::Zone(this as usize, x, z));
        // SAFETY: the test's zone manager.
        let m = unsafe { &mut *this };
        if let Some((lv, next)) = m.swap.take() {
            // SAFETY: the test's simulator object.
            unsafe { LandValueSim::from_ptr(lv) }.set_zone_manager(next);
        }
        m.zoned
            .iter()
            .find(|&&(zx, zz, _)| zx == x && zz == z)
            .map_or(0, |&(_, _, t)| t)
    }

    fn vtable(slots: &[(usize, usize)]) -> Box<[usize; 16]> {
        let mut v = Box::new([0usize; 16]);
        for &(offset, f) in slots {
            v[offset / 4] = f;
        }
        v
    }

    struct World {
        lv: Object,
        grid: Box<FakeGrid>,
        zones: Box<FakeZones>,
        _grid_vt: Box<[usize; 16]>,
        _zones_vt: Box<[usize; 16]>,
    }

    impl World {
        fn new(shift: u32, size: i32, zoned: &[(i32, i32, u32)]) -> World {
            let grid_vt = vtable(&[
                (GRID_TRACT_SIZE, grid_size as *const () as usize),
                (GRID_TRACT_SHIFT, grid_shift as *const () as usize),
            ]);
            let zones_vt = vtable(&[(ZONE_TYPE, zone_type as *const () as usize)]);
            let grid = Box::new(FakeGrid {
                vtable: grid_vt.as_ptr(),
                shift,
                size,
                swap: None,
            });
            let zones = Box::new(FakeZones {
                vtable: zones_vt.as_ptr(),
                zoned: zoned.to_vec(),
                swap: None,
            });
            let mut w = World {
                lv: Object::new(0x100),
                grid,
                zones,
                _grid_vt: grid_vt,
                _zones_vt: zones_vt,
            };
            let lv = w.sim();
            lv.set_intrinsic_grid((&mut *w.grid as *mut FakeGrid).cast());
            lv.set_zone_manager((&mut *w.zones as *mut FakeZones).cast());
            take_calls();
            w
        }
        fn sim(&mut self) -> LandValueSim {
            // SAFETY: 0x100 zeroed bytes cover the layout.
            unsafe { LandValueSim::from_ptr(self.lv.ptr()) }
        }
        fn grid_addr(&self) -> usize {
            &*self.grid as *const FakeGrid as usize
        }
        fn zones_addr(&self) -> usize {
            &*self.zones as *const FakeZones as usize
        }
        fn is_zoned(&mut self, tx: i32, tz: i32) -> u32 {
            // SAFETY: the world's objects.
            unsafe { fc4p_006bd540(self.lv.ptr(), tx, tz) }
        }
    }

    #[test]
    fn zoned_scans_cells_x_outer_and_stops_at_the_first() {
        // S = 2: tract (3, 4) covers cells x 6..=7, z 8..=9.
        let mut w = World::new(1, 2, &[(7, 9, 1), (7, 8, 3)]);
        let (g, m) = (w.grid_addr(), w.zones_addr());
        assert_eq!(w.is_zoned(3, 4) & 0xFF, 1);
        assert_eq!(
            take_calls(),
            [
                Call::Shift(g),
                Call::Size(g),
                Call::Zone(m, 6, 8),
                Call::Zone(m, 6, 9),
                Call::Zone(m, 7, 8)
            ]
        );
    }

    #[test]
    fn unzoned_tract_visits_every_cell_once() {
        let mut w = World::new(1, 2, &[(5, 9, 1), (8, 8, 1), (6, 10, 1)]);
        let (g, m) = (w.grid_addr(), w.zones_addr());
        let eax = w.is_zoned(3, 4);
        assert_eq!(eax & 0xFF, 0);
        let mut want = vec![Call::Shift(g), Call::Size(g)];
        for x in 6..8 {
            for z in 8..10 {
                want.push(Call::Zone(m, x, z));
            }
        }
        assert_eq!(take_calls(), want);
    }

    #[test]
    fn zone_type_compares_all_32_bits() {
        // A zone type with a zero low byte still counts; AL is 1 and the upper bytes are the zone type's.
        let mut w = World::new(1, 2, &[(0, 1, 0x0001_2300)]);
        assert_eq!(w.is_zoned(0, 0), 0x0001_2301);
    }

    #[test]
    fn eax_upper_bytes_when_not_zoned() {
        // tz·S + S = 200·4 + 4 = 804 = 0x324: AL 0, upper bytes 0x300.
        let mut w = World::new(2, 4, &[]);
        assert_eq!(w.is_zoned(1, 200), 0x300);
        // Size 0: no zone calls, same rule (tz·S = 0x324 again with tz = 201, S = 4, size 0).
        let mut w = World::new(2, 0, &[]);
        let g = w.grid_addr();
        assert_eq!(w.is_zoned(1, 201), 0x300);
        assert_eq!(take_calls(), [Call::Shift(g), Call::Size(g)]);
    }

    #[test]
    fn bounds_wrap_and_shift_is_modulo_32() {
        // Shift 33 acts as 1. tx = 0x4000_0000 << 1 wraps to i32::MIN; the cells still run x0..x0+2.
        let mut w = World::new(33, 2, &[(i32::MIN + 1, 2, 7)]);
        let m = w.zones_addr();
        assert_eq!(w.is_zoned(0x4000_0000, 1), 1);
        let calls = take_calls();
        assert_eq!(
            calls[2..],
            [
                Call::Zone(m, i32::MIN, 2),
                Call::Zone(m, i32::MIN, 3),
                Call::Zone(m, i32::MIN + 1, 2)
            ]
        );
        // x0 + S overflowing makes x0 >= x1: no cell is visited.
        let mut w = World::new(0, 2, &[]);
        assert_eq!(w.is_zoned(i32::MAX - 1, 0) & 0xFF, 0);
        assert_eq!(take_calls().len(), 2);
    }

    #[test]
    fn grid_and_zone_manager_are_read_before_each_call() {
        let mut w = World::new(1, 2, &[]);
        let other_grid_vt = vtable(&[
            (GRID_TRACT_SIZE, grid_size as *const () as usize),
            (GRID_TRACT_SHIFT, grid_shift as *const () as usize),
        ]);
        let mut other_grid = Box::new(FakeGrid {
            vtable: other_grid_vt.as_ptr(),
            shift: 9,
            size: 3,
            swap: None,
        });
        let other_zones_vt = vtable(&[(ZONE_TYPE, zone_type as *const () as usize)]);
        let mut other_zones = Box::new(FakeZones {
            vtable: other_zones_vt.as_ptr(),
            zoned: vec![(0, 1, 0x0500)],
            swap: None,
        });
        let lv = w.lv.ptr();
        let (og, oz) = (
            (&mut *other_grid as *mut FakeGrid),
            (&mut *other_zones as *mut FakeZones),
        );
        w.grid.swap = Some((lv, og.cast()));
        w.zones.swap = Some((lv, oz.cast()));
        let (g, m) = (w.grid_addr(), w.zones_addr());
        // The shift comes from the first grid (1), the size from the second (3); the first cell's zone type from
        // the first manager (0), the second's from the second (0x500).
        assert_eq!(w.is_zoned(0, 0), 0x0501);
        assert_eq!(
            take_calls(),
            [
                Call::Shift(g),
                Call::Size(og as usize),
                Call::Zone(m, 0, 0),
                Call::Zone(oz as usize, 0, 1)
            ]
        );
    }

    game_fn! {
        /// The original's address, which tests resolve to our port.
        fn tract_is_zoned_by_address(this: *mut u8, tx: i32, tz: i32) -> u32 = "thiscall" 0x006B_D540;
    }

    #[test]
    fn a_game_function_with_a_port_resolves_to_the_port() {
        let mut w = World::new(1, 2, &[(2, 2, 1)]);
        // SAFETY: the world's objects.
        assert_eq!(unsafe { tract_is_zoned_by_address(w.lv.ptr(), 1, 1) } & 0xFF, 1);
    }

    // ---- altitude effect ------------------------------------------------------------------------------

    /// A curve: points (x, y), linear between them, the end values outside; or a constant.
    #[repr(C)]
    struct FakeCurve {
        points: Vec<(f32, f32)>,
        constant: Option<f32>,
        identity: bool,
    }

    extern "thiscall" fn curve_output(this: *mut FakeCurve, x: f32) -> f32 {
        log(Call::Curve(this as usize, x));
        // SAFETY: the test's curve.
        let c = unsafe { &*this };
        if let Some(v) = c.constant {
            return v;
        }
        if c.identity {
            return x;
        }
        let p = &c.points;
        if x <= p[0].0 {
            return p[0].1;
        }
        for w in p.windows(2) {
            if x <= w[1].0 {
                return w[0].1 + (x - w[0].0) * (w[1].1 - w[0].1) / (w[1].0 - w[0].0);
            }
        }
        p[p.len() - 1].1
    }

    /// The shipped altitude curve (§7.3).
    const SHIPPED: [(f32, f32); 7] = [
        (0.0, 0.0),
        (25.0, 5.0),
        (50.0, 10.0),
        (75.0, 21.0),
        (100.0, 50.0),
        (125.0, 30.0),
        (150.0, 0.0),
    ];

    struct Altitude {
        lv: Object,
        curve: Box<FakeCurve>,
        _fake: crate::testing::Fake,
        _fpu: GameFpu,
    }

    impl Altitude {
        /// The parameters as SetupAltitudeEffect leaves them.
        fn new(min: f32, pivot: f32, low: f32, high: f32, top: i32, curve: FakeCurve) -> Altitude {
            let mut a = Altitude {
                lv: Object::new(0x100),
                curve: Box::new(curve),
                _fake: fake(0x005F_4250, curve_output as *const () as usize),
                _fpu: GameFpu::new(),
            };
            let curve_ptr = (&mut *a.curve as *mut FakeCurve).cast();
            let lv = a.sim();
            lv.set_min_altitude(min);
            lv.set_pivot(pivot);
            lv.set_low_range(low);
            lv.set_high_range(high);
            lv.set_top_index(top);
            lv.set_altitude_curve(curve_ptr);
            take_calls();
            a
        }
        fn sim(&mut self) -> LandValueSim {
            // SAFETY: 0x100 zeroed bytes cover the layout.
            unsafe { LandValueSim::from_ptr(self.lv.ptr()) }
        }
        /// EAX after the call with EAX = `entry` at entry.
        fn effect_eax(&mut self, a: f32, entry: u32) -> u32 {
            // SAFETY: the export takes `this` and one float.
            unsafe {
                call_thiscall_with_eax(
                    fc4p_006bd810 as *const () as usize,
                    self.lv.ptr(),
                    &[a.to_bits()],
                    entry,
                )
                .0
            }
        }
        fn effect(&mut self, a: f32) -> u8 {
            self.effect_eax(a, 0) as u8
        }
    }

    fn shipped() -> FakeCurve {
        FakeCurve {
            points: SHIPPED.to_vec(),
            constant: None,
            identity: false,
        }
    }

    fn constant(v: f32) -> FakeCurve {
        FakeCurve {
            points: Vec::new(),
            constant: Some(v),
            identity: false,
        }
    }

    fn identity() -> FakeCurve {
        FakeCurve {
            points: Vec::new(),
            constant: None,
            identity: true,
        }
    }

    #[test]
    fn shipped_curve_effects() {
        // lv16_08: minAlt 260, maxAlt 322, mode 272, mean 300 -> pivot 272, lowRange 12, highRange 50, topIndex 100.
        let mut a = Altitude::new(260.0, 272.0, 12.0, 50.0, 100, shipped());
        let c = &*a.curve as *const FakeCurve as usize;
        for (alt, effect, index) in [
            (260.0, 0, 0.0),
            (263.0, 2, 12.0),
            (266.0, 5, 25.0),
            (272.0, 10, 50.0),
            (297.0, 21, 75.0),
            (322.0, 50, 100.0),
        ] {
            assert_eq!(a.effect(alt), effect, "AltitudeEffect({alt})");
            assert_eq!(take_calls(), [Call::Curve(c, index)], "one curve call at index {index}");
        }
        assert_eq!(a.effect(259.0), 0);
        assert_eq!(take_calls(), [], "below minAlt: no curve call");
    }

    #[test]
    fn flat_terrain_effects() {
        // setup [300, 300, 300, 300]: pivot 300, ranges 1, topIndex 100.
        let mut a = Altitude::new(300.0, 300.0, 1.0, 1.0, 100, shipped());
        assert_eq!(a.effect(300.0), 10);
        assert_eq!(a.effect(299.0), 0);
    }

    #[test]
    fn curve_output_wraps_to_8_bits() {
        // setup [100, 100, 0, 200]: pivot 100, minAlt 0, ranges 100, topIndex 100.
        let mut a = Altitude::new(0.0, 100.0, 100.0, 100.0, 100, constant(-10.0));
        assert_eq!(a.effect(150.0), 246);
        assert_eq!(a.effect(10.0), 246);
        assert_eq!(a.effect_eax(10.0, 0), 0xFFFF_FFF6);
        let mut a = Altitude::new(0.0, 100.0, 100.0, 100.0, 100, constant(300.0));
        assert_eq!(a.effect(150.0), 44);
        assert_eq!(a.effect_eax(150.0, 0), 300);
    }

    #[test]
    fn truncation_is_the_games_helper() {
        // 3e9 keeps its low 32 bits; 1e19 and NaN give the integer indefinite's low half, 0.
        let mut a = Altitude::new(0.0, 100.0, 100.0, 100.0, 100, constant(3e9));
        assert_eq!(a.effect_eax(150.0, 0), 3_000_000_000);
        let mut a = Altitude::new(0.0, 100.0, 100.0, 100.0, 100, constant(1e19));
        assert_eq!(a.effect_eax(150.0, 0), 0);
        let mut a = Altitude::new(0.0, 100.0, 100.0, 100.0, 100, constant(f32::NAN));
        assert_eq!(a.effect_eax(150.0, 0), 0);
    }

    #[test]
    fn nan_altitude_takes_the_upper_branch() {
        // NaN is not below minAlt and not below the pivot: index trunc(NaN) = indefinite, low half 0.
        let mut a = Altitude::new(0.0, 100.0, 100.0, 100.0, 100, shipped());
        let c = &*a.curve as *const FakeCurve as usize;
        assert_eq!(a.effect(f32::NAN), 0);
        assert_eq!(take_calls(), [Call::Curve(c, 0.0)]);
    }

    #[test]
    fn below_min_keeps_the_callers_eax_upper_half() {
        let mut a = Altitude::new(260.0, 272.0, 12.0, 50.0, 100, shipped());
        // AH: the status word's high byte after the comparison (C0 set, stack top 0).
        assert_eq!(a.effect_eax(259.0, 0xABCD_1234), 0xABCD_0100);
        // Otherwise EAX is the helper's result whatever it was at entry.
        assert_eq!(a.effect_eax(272.0, 0xABCD_1234), 10);
    }

    #[test]
    fn integer_load_is_exact_and_rounding_is_24_bit() {
        // topIndex − 50 = 2147483597 loads exactly; /3 rounds to 715827840 at 24 bits; +50 rounds to 715827904.
        // In f32 arithmetic the integer would round to 2^31 first and the result would be 715827968.
        let mut a = Altitude::new(-1000.0, 0.0, 1.0, 3.0, i32::MAX, identity());
        let c = &*a.curve as *const FakeCurve as usize;
        assert_eq!(a.effect_eax(1.0, 0), 715_827_904);
        assert_eq!(take_calls(), [Call::Curve(c, 715_827_904.0)]);
    }

    #[test]
    fn intermediate_beyond_f32_range_comes_back() {
        // highRange 1e-38: the slope 50 / 1e-38 is beyond f32 but not the register; × (a − pivot) = 1e-29 brings it
        // back to about 5e10, whose low 32 bits are the index. In f32 arithmetic the slope is infinite: index 0.
        let mut a = Altitude::new(-1.0, 0.0, 1.0, 1e-38, 100, identity());
        let c = &*a.curve as *const FakeCurve as usize;
        let eax = a.effect_eax(1e-29, 0);
        let calls = take_calls();
        let Call::Curve(cc, index) = calls[0] else {
            panic!("{calls:?}")
        };
        assert_eq!(cc, c);
        let wide = 50.0f64 / f64::from(1e-38f32) * f64::from(1e-29f32) + 50.0;
        let low = (wide as i64) as i32;
        assert!(
            (f64::from(index) - f64::from(low)).abs() < 4096.0,
            "index {index} vs about {low}"
        );
        assert_eq!(eax, index as i32 as u32);
        assert_ne!(eax, 0);
    }

    #[test]
    fn layout_offsets() {
        let names: Vec<_> = LandValueSim::FIELDS.iter().map(|&(n, o, _)| (n, o)).collect();
        assert!(names.contains(&("zone_manager", 0x20)));
        assert!(names.contains(&("intrinsic_grid", 0x24)));
        assert!(names.contains(&("altitude_curve", 0x84)));
        let mut spans: Vec<_> = LandValueSim::FIELDS.iter().map(|&(_, o, s)| (o, o + s)).collect();
        spans.sort();
        assert!(spans.windows(2).all(|w| w[0].1 <= w[1].0), "fields overlap");
    }
}
