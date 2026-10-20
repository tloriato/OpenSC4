// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Conformance ops for `docs/specs/effects.md` §17: the brush stroke counts,
//! the select draw, the game-record action order, the priority filter, the
//! finish rule and the shipped flag and LOD censuses.
//!
//! These run over `fc4_core::effects` with no city and no host, which is what
//! §17 asks for: "conformance tests (must exist in `engine/conformance` if the
//! manager or the brush effect is replaced)".

use fc4_core::effects::brush::{floor_count, BrushEffect, EffectsRandom};
use fc4_core::effects::directory::{ComponentDesc, EffectDesc, EffectDirectory, Found, NameEntry};
use fc4_core::effects::game::GameEffect;
use fc4_core::effects::manager::{ComponentState, EffectsManager};
use fc4_core::effects::select::{draw_select, select_components, select_generator, SelectPlacement};
use fc4_core::effects::shipped::{game_flags, BrushRecord, BRUSH_RECORDS, GAME_RECORDS};
use fc4_core::effects::{component_type, runtime_flags, EffectRuntime, RuntimeComponent, SweepResult};

use crate::json::Json;

type R<T> = Result<T, String>;

fn field<'a>(op: &'a Json, key: &str) -> R<&'a Json> {
    op.get(key).ok_or_else(|| format!("missing field '{key}'"))
}

fn int(j: &Json, what: &str) -> R<i64> {
    j.as_i64().ok_or_else(|| format!("'{what}' must be an integer"))
}

fn num(j: &Json, what: &str) -> R<f64> {
    j.as_f64().ok_or_else(|| format!("'{what}' must be a number"))
}

fn text<'a>(op: &'a Json, key: &str) -> R<&'a str> {
    field(op, key)?
        .as_str()
        .ok_or_else(|| format!("'{key}' must be a string"))
}

/// Compares one integer field of `op` with `got`, when the field is present.
fn expect_int(op: &Json, key: &str, got: i64) -> R<()> {
    if let Some(want) = op.get(key) {
        let want = int(want, key)?;
        if want != got {
            return Err(format!("{key} = {got}, expected {want}"));
        }
    }
    Ok(())
}

/// Compares one float field of `op` with `got`, when the field is present.
fn expect_f64(op: &Json, key: &str, got: f64) -> R<()> {
    if let Some(want) = op.get(key) {
        let want = num(want, key)?;
        if (want - got).abs() > 1e-4 {
            return Err(format!("{key} = {got}, expected {want}"));
        }
    }
    Ok(())
}

/// A generator whose `RandomDoubleRangeUniform` returns `value`.
struct FixedRandom(f64);
impl EffectsRandom for FixedRandom {
    fn range_f64(&mut self, a: f64, b: f64) -> f64 {
        if (a - b).abs() < f64::EPSILON {
            a
        } else {
            self.0
        }
    }
}

/// Executes an effects op; `None` if `name` is not one.
pub fn exec(name: &str, op: &Json) -> Option<R<()>> {
    let r = match name {
        "ef_runtime_flags" => runtime_flags_op(op),
        "ef_finish_rule" => finish_rule(op),
        "ef_select" => select_op(op),
        "ef_brush_stroke" => brush_stroke(op),
        "ef_brush_one_shot" => brush_one_shot(op),
        "ef_brush_bounds" => brush_bounds(op),
        "ef_game_actions" => game_actions(op),
        "ef_game_pause" => game_pause(op),
        "ef_priority" => priority(op),
        "ef_lod" => lod_op(op),
        "ef_shipped_counts" => shipped_counts(op),
        "ef_meta_waits" => meta_waits(op),
        _ => return None,
    };
    Some(r)
}

/// §17 test 12: the description bit is not the tested bit. `desc+0x10 & 1` is
/// copied to runtime bit 2 (value 4), and table types 3-15 also get bit 1
/// (value 2).
fn runtime_flags_op(op: &Json) -> R<()> {
    let desc_flags = int(field(op, "desc_flags")?, "desc_flags")? as u32;
    let kind = int(field(op, "type")?, "type")? as u8;
    let got = runtime_flags(desc_flags, kind);
    expect_int(op, "runtime_flags", i64::from(got))?;
    if let Some(want) = op.get("ignored") {
        let ignored = got & 4 != 0;
        if want.as_bool().ok_or("'ignored' must be a bool")? != ignored {
            return Err(format!("ignored = {ignored}"));
        }
    }
    Ok(())
}

/// §17 test 11: the finish rule of §7.
fn finish_rule(op: &Json) -> R<()> {
    let started = bool_field(op, "started", true)?;
    let index = int(op.get("index").unwrap_or(&Json::Int(3)), "index")? as i32;
    let flags = int(op.get("flags").unwrap_or(&Json::Int(0)), "flags")? as u32;
    let stopping = bool_field(op, "stopping", false)?;
    let end_type = int(op.get("end_message_type").unwrap_or(&Json::Int(0)), "end_message_type")? as u32;

    let runtime = EffectRuntime {
        index,
        started,
        stopping,
        flags,
        end_message_type: end_type,
        end_message_data1: 0,
    };
    let mut components = Vec::new();
    for c in field(op, "components")?
        .as_array()
        .ok_or("'components' must be an array")?
    {
        components.push(RuntimeComponent {
            flags: int(c.get("flags").unwrap_or(&Json::Int(0)), "flags")? as u8,
            is_active: c
                .get("active")
                .and_then(|v| v.as_bool())
                .ok_or("each component needs 'active'")?,
        });
    }
    let want = text(op, "expect")?;
    let got = match runtime.sweep(&components) {
        SweepResult::DoneWithoutMessage => "done_without_message",
        SweepResult::StillRunning => "still_running",
        SweepResult::Finished { post_end_message } => {
            if post_end_message {
                "finished_with_message"
            } else {
                "finished_without_message"
            }
        }
    };
    if got != want {
        return Err(format!("sweep = {got}, expected {want}"));
    }
    Ok(())
}

fn bool_field(op: &Json, key: &str, default: bool) -> R<bool> {
    Ok(match op.get(key) {
        Some(j) => j.as_bool().ok_or_else(|| format!("'{key}' must be a bool"))?,
        None => default,
    })
}

/// §17 test 3: the select draw of §5.3.
fn select_op(op: &Json) -> R<()> {
    // A fresh generator, seeded 0x1105.
    let mut g = select_generator();
    if let Some(want) = op.get("first_word") {
        let got = g.next_u32();
        let want = want.as_str().ok_or("'first_word' must be a hex string")?;
        let want = u32::from_str_radix(want.trim_start_matches("0x").trim_start_matches("0X"), 16)
            .map_err(|e| e.to_string())?;
        if got != want {
            return Err(format!("first tempered word = {got:#010X}, expected {want:#010X}"));
        }
        return Ok(());
    }

    let weights: Vec<u16> = field(op, "weights")?
        .as_array()
        .ok_or("'weights' must be an array")?
        .iter()
        .map(|w| int(w, "weight").map(|v| v as u16))
        .collect::<R<Vec<u16>>>()?;
    let placements: Vec<SelectPlacement> = weights.iter().map(|&w| SelectPlacement::member(1, w)).collect();

    // Explicit draws, or the generator's own.
    let created = if let Some(draws) = op.get("draws") {
        let mut ds = draws
            .as_array()
            .ok_or("'draws' must be an array")?
            .iter()
            .map(|d| int(d, "draw").map(|v| v as u32))
            .collect::<R<Vec<u32>>>()?
            .into_iter();
        select_components(&placements, move || ds.next().unwrap_or(u32::MAX))
    } else {
        let mut g = select_generator();
        select_components(&placements, || draw_select(&mut g))
    };

    let chosen: Vec<i64> = created
        .iter()
        .enumerate()
        .filter(|(_, &c)| c)
        .map(|(i, _)| i as i64 + 1)
        .collect();
    let want: Vec<i64> = field(op, "expect_members")?
        .as_array()
        .ok_or("'expect_members' must be an array")?
        .iter()
        .map(|m| int(m, "member"))
        .collect::<R<Vec<i64>>>()?;
    if chosen != want {
        return Err(format!("members {chosen:?}, expected {want:?}"));
    }
    Ok(())
}

/// §17 tests 1 and 2: the brush stroke's application count.
fn brush_stroke(op: &Json) -> R<()> {
    let record = brush_record(op)?;
    let us = int(field(op, "us")?, "us")? as f64;
    let dilation = num(op.get("dilation").unwrap_or(&Json::Float(1.0)), "dilation")?;
    let ticks_max = int(field(op, "ticks")?, "ticks")?.max(0) as u32;

    // §6: dt' = trunc(r24(us * dilation)); dt = r24(dt' * 1e-6).
    let dt_prime = fc4_core::x87::ftol32(fc4_core::x87::r24(us * dilation)).max(0) as u64;
    expect_int(op, "dt_prime", dt_prime as i64)?;
    let dt = fc4_core::x87::r24(dt_prime as f64 * 1e-6) as f32;
    expect_f64(op, "dt", f64::from(dt))?;

    let mut b = BrushEffect::new(record, &mut FixedRandom(0.0));
    b.set_position(100.0, 100.0);
    b.start();

    let mut per_tick = Vec::new();
    let mut total = 0u32;
    let mut ticks = 0u32;
    while b.is_active() && ticks < ticks_max {
        ticks += 1;
        let n = b.update_stroke(dt);
        per_tick.push(n);
        total += n;
        let _ = b.applications(n, |_, _| true);
    }
    expect_int(op, "tick_count", i64::from(ticks))?;
    expect_int(op, "applications", i64::from(total))?;
    if let Some(want) = op.get("per_tick") {
        let want: Vec<i64> = want
            .as_array()
            .ok_or("'per_tick' must be an array")?
            .iter()
            .map(|v| int(v, "n"))
            .collect::<R<Vec<i64>>>()?;
        let got: Vec<i64> = per_tick.iter().take(want.len()).map(|&n| n as i64).collect();
        if got != want {
            return Err(format!("per-tick n = {got:?}, expected {want:?}"));
        }
    }
    expect_f64(op, "t", f64::from(b.t))?;
    expect_f64(op, "acc", f64::from(b.acc))?;
    Ok(())
}

/// §17 test 4: a one-shot crater.
fn brush_one_shot(op: &Json) -> R<()> {
    let record = brush_record(op)?;
    let mut b = BrushEffect::new(record, &mut FixedRandom(0.0));
    b.set_position(10.0, 20.0);
    b.start();
    b.cur = (11.0, 21.0);
    let n = b.update_one_shot();
    expect_int(op, "applications", i64::from(n))?;
    if b.is_active() {
        return Err("a one-shot is inactive after its first update".into());
    }
    if b.long_operation_open() || b.needs_long_operation() {
        return Err("a one-shot opens no long terrain operation".into());
    }
    let apps = b.applications(n, |_, _| true);
    expect_int(op, "calls", apps.len() as i64)?;
    if let Some(want) = op.get("redisplay") {
        let want: Vec<bool> = want
            .as_array()
            .ok_or("'redisplay' must be an array")?
            .iter()
            .map(|v| v.as_bool().ok_or_else(|| "each value must be a bool".to_string()))
            .collect::<R<Vec<bool>>>()?;
        let got: Vec<bool> = apps.iter().map(|a| a.redisplay).collect();
        if got != want {
            return Err(format!("redisplay = {got:?}, expected {want:?}"));
        }
    }
    for (key, got) in [
        ("level", apps.first().map(|a| i64::from(a.level)).unwrap_or(-1)),
        (
            "strength_x100",
            apps.first().map(|a| (a.strength * 100.0) as i64).unwrap_or(-1),
        ),
        (
            "radius_x100",
            apps.first().map(|a| (a.radius_scale * 100.0) as i64).unwrap_or(-1),
        ),
        ("data4", apps.first().map(|a| i64::from(a.data4)).unwrap_or(-1)),
    ] {
        expect_int(op, key, got)?;
    }
    Ok(())
}

/// §17 test 5: out of bounds applies nothing and keeps `prev`.
fn brush_bounds(op: &Json) -> R<()> {
    let record = brush_record(op)?;
    let mut b = BrushEffect::new(record, &mut FixedRandom(0.0));
    let ox = num(field(op, "prev_x")?, "prev_x")? as f32;
    let oz = num(field(op, "prev_z")?, "prev_z")? as f32;
    b.set_position(ox, oz);
    b.start();
    let cx = num(field(op, "cur_x")?, "cur_x")? as f32;
    let cz = num(field(op, "cur_z")?, "cur_z")? as f32;
    b.cur = (cx, cz);

    let cells = (
        int(field(op, "cells_x")?, "cells_x")? as u32,
        int(field(op, "cells_z")?, "cells_z")? as u32,
    );
    let in_city = |x: f32, z: f32| x >= 0.0 && z >= 0.0 && x < cells.0 as f32 * 16.0 && z < cells.1 as f32 * 16.0;
    let apps = b.applications(int(field(op, "n")?, "n")? as u32, in_city);
    expect_int(op, "calls", apps.len() as i64)?;
    expect_f64(op, "prev_x_after", f64::from(b.prev.0))?;
    expect_f64(op, "prev_z_after", f64::from(b.prev.1))?;
    Ok(())
}

/// §17 tests 6 and 7: a game record's action order and the move-to-a-new-cell
/// rule.
fn game_actions(op: &Json) -> R<()> {
    let index = int(field(op, "record")?, "record")? as usize;
    let record = *GAME_RECORDS.get(index).ok_or("no such game record")?;
    let mut g = GameEffect::new(record);
    let x = num(field(op, "x")?, "x")? as f32;
    let z = num(field(op, "z")?, "z")? as f32;
    g.set_position(x, z);
    if !g.start() {
        return Err("the record did not start".into());
    }
    if let Some(data) = op.get("data") {
        g.set_parameter(13, int(data, "data")? as u64);
    }
    let cells = (
        int(field(op, "cells_x")?, "cells_x")? as u32,
        int(field(op, "cells_z")?, "cells_z")? as u32,
    );

    let mut names = Vec::new();
    let updates = int(op.get("updates").unwrap_or(&Json::Int(1)), "updates")? as usize;
    for _ in 0..updates {
        let u = g.update(1.0, cells);
        for a in &u.actions {
            names.push(action_name(a));
        }
    }
    let want: Vec<String> = field(op, "expect_actions")?
        .as_array()
        .ok_or("'expect_actions' must be an array")?
        .iter()
        .map(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| "each value must be a string".to_string())
        })
        .collect::<R<Vec<String>>>()?;
    if names != want {
        return Err(format!("actions {names:?}, expected {want:?}"));
    }
    expect_int(op, "count", i64::from(g.count))?;
    expect_int(op, "cell_x", i64::from(g.cell.0))?;
    expect_int(op, "cell_z", i64::from(g.cell.1))?;
    Ok(())
}

fn action_name(a: &fc4_core::effects::game::GameAction) -> String {
    use fc4_core::effects::game::GameAction::*;
    match a {
        Burn { radius, .. } => format!("burn{radius}"),
        SetFireCause { cause } => format!("cause{cause}"),
        Toxic { .. } => "toxic".into(),
        Demolish {
            x0,
            z0,
            x1,
            z1,
            flags,
            dezone,
        } => format!("demolish({x0},{z0},{x1},{z1},{flags},{dezone})"),
        Extinguish { amount, .. } => format!("extinguish{amount}"),
        Message { msg_type, .. } => format!("message{msg_type:08X}"),
        Map { .. } => "map".into(),
    }
}

/// §17 test 8: a pause record.
fn game_pause(op: &Json) -> R<()> {
    let index = int(field(op, "record")?, "record")? as usize;
    let record = *GAME_RECORDS.get(index).ok_or("no such game record")?;
    let mut g = GameEffect::new(record);
    if !g.start() {
        return Err("the record did not start".into());
    }
    let start_pauses: Vec<String> = g.start_pauses().iter().map(|p| format!("{p:?}")).collect();
    let want: Vec<String> = field(op, "expect_start_pauses")?
        .as_array()
        .ok_or("'expect_start_pauses' must be an array")?
        .iter()
        .map(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| "each value must be a string".to_string())
        })
        .collect::<R<Vec<String>>>()?;
    if start_pauses != want {
        return Err(format!("start pauses {start_pauses:?}, expected {want:?}"));
    }

    let updates = int(field(op, "updates")?, "updates")? as usize;
    let mut undo = Vec::new();
    for _ in 0..updates {
        let u = g.update(1.0, (64, 64));
        undo.extend(u.undo_pauses.iter().map(|p| format!("{p:?}")));
    }
    if let Some(want) = op.get("expect_undo_pauses") {
        let want: Vec<String> = want
            .as_array()
            .ok_or("'expect_undo_pauses' must be an array")?
            .iter()
            .map(|v| {
                v.as_str()
                    .map(|s| s.to_string())
                    .ok_or_else(|| "each value must be a string".to_string())
            })
            .collect::<R<Vec<String>>>()?;
        if undo != want {
            return Err(format!("undo pauses {undo:?}, expected {want:?}"));
        }
    }
    expect_int(op, "timer_x1000", (f64::from(g.timer) * 1000.0).round() as i64)?;
    if let Some(want) = op.get("active") {
        let active = g.is_active();
        if want.as_bool().ok_or("'active' must be a bool")? != active {
            return Err(format!("active = {active}"));
        }
    }
    if let Some(want) = op.get("second_stop") {
        let got = g.stop();
        if want.as_bool().ok_or("'second_stop' must be a bool")? != got {
            return Err(format!("second stop = {got}"));
        }
    }
    Ok(())
}

/// §17 test 9: the priority filter of §4.1.
fn priority(op: &Json) -> R<()> {
    let mut descriptions = Vec::new();
    let mut names = Vec::new();
    for (i, d) in field(op, "descriptions")?
        .as_array()
        .ok_or("'descriptions' must be an array")?
        .iter()
        .enumerate()
    {
        let name = d
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("each description needs 'name'")?;
        let pri = int(d.get("priority").unwrap_or(&Json::Int(1)), "priority")? as u32;
        descriptions.push(EffectDesc {
            flags: 0,
            priority: pri,
            components: Vec::new(),
            name: name.as_bytes().to_vec(),
        });
        names.push(NameEntry {
            name: name.as_bytes().to_vec(),
            description_index: i as u32,
        });
    }
    let dir = EffectDirectory::from_parts(descriptions, names);
    let max_priority = int(field(op, "max_priority")?, "max_priority")? as u32;
    let name = text(op, "name")?.as_bytes().to_vec();
    let found = dir.find(&fc4_core::effects::directory::lowercase(&name), max_priority);
    let mut m = EffectsManager::new(dir);
    m.set_max_priority(max_priority);

    match found {
        Found::Index(i) => expect_int(op, "expect_index", i64::from(i))?,
        Found::BelowPriority => expect_int(op, "expect_index", -1)?,
        Found::NoSuchName => {
            if op.get("expect_index").is_some() {
                return Err("the name does not exist".into());
            }
        }
    }
    Ok(())
}

/// §17 test 14: the LOD census of §5.2.
fn lod_op(op: &Json) -> R<()> {
    let min = int(field(op, "min")?, "min")? as u8;
    let max = int(field(op, "max")?, "max")? as u8;
    let c = ComponentDesc {
        name: Vec::new(),
        kind: int(field(op, "type")?, "type")? as u8,
        flags: 0,
        lod_min: min,
        lod_max: max,
        select_group: 0,
        select_weight: 0,
        index: 0,
    };
    if let Some(want) = op.get("reaches_table") {
        let got = c.reaches_table();
        if want.as_bool().ok_or("'reaches_table' must be a bool")? != got {
            return Err(format!("reaches_table = {got}"));
        }
    }
    for lod in [1u8, 2, 3, 4, 5] {
        if let Some(want) = op.get(format!("at_{lod}").as_str()) {
            let got = c.exists_at(lod);
            if want.as_bool().ok_or("'at_N' must be a bool")? != got {
                return Err(format!("exists at {lod} = {got}"));
            }
        }
    }
    Ok(())
}

/// §17 tests 13 and 14: the shipped counts.
fn shipped_counts(op: &Json) -> R<()> {
    expect_int(op, "brush_records", BRUSH_RECORDS.len() as i64)?;
    expect_int(op, "game_records", GAME_RECORDS.len() as i64)?;
    expect_int(
        op,
        "game_no_action",
        GAME_RECORDS
            .iter()
            .filter(|g| {
                g.flags & 7 == 0
                    && g.demolish_radius == 0
                    && g.burn_radius == 0
                    && g.toxic_amount == 0
                    && g.extinguish_amount == 0
                    && g.message_type == 0
                    && g.duration == 0.0
            })
            .count() as i64,
    )?;
    expect_int(
        op,
        "game_single",
        GAME_RECORDS
            .iter()
            .filter(|g| g.flags & game_flags::SINGLE != 0)
            .count() as i64,
    )?;
    Ok(())
}

/// §17 test 15: `volcano_control` finishes only after the chosen child finished
/// on a previous sweep **and** its brush, particle and decal components are all
/// inactive on this one.
fn meta_waits(op: &Json) -> R<()> {
    let finished_earlier = bool_field(op, "child_finished_earlier", false)?;
    let brush = bool_field(op, "brush_active", false)?;
    let particle = bool_field(op, "particle_active", false)?;
    let decal = bool_field(op, "decal_active", false)?;
    let attractor = bool_field(op, "attractor_active", false)?;
    let game = bool_field(op, "game_active", false)?;

    let child = fc4_core::effects::VolcanoCreatorChild {
        finished_on_an_earlier_sweep: finished_earlier,
        brush_active: brush,
        particle_active: particle,
        decal_active: decal,
        attractor_active: attractor,
        game_active: game,
    };
    let got = child.control_finishes();
    let want = bool_field(op, "expect_finishes", false)?;
    if got != want {
        return Err(format!("control_finishes = {got}, expected {want}"));
    }
    Ok(())
}

/// The brush record named by an op: either a shipped index or an explicit one.
fn brush_record(op: &Json) -> R<BrushRecord> {
    if let Some(i) = op.get("record") {
        let i = int(i, "record")? as usize;
        return Ok(*BRUSH_RECORDS.get(i).ok_or("no such brush record")?);
    }
    Ok(BrushRecord {
        brush_id: int(field(op, "brush_id")?, "brush_id")? as u32,
        rate: num(field(op, "rate")?, "rate")? as f32,
        length: num(field(op, "length")?, "length")? as f32,
        one_shot: bool_field(op, "one_shot", false)?,
        level: int(field(op, "level")?, "level")? as u32,
        strength: [num(field(op, "strength")?, "strength")? as f32; 2],
        radius_scale: [num(field(op, "radius")?, "radius")? as f32; 2],
        target_height: num(op.get("target").unwrap_or(&Json::Float(100.0)), "target")? as f32,
    })
}

/// Kept for the module to compile with the same shape as its siblings.
const _: fn() = || {
    let _ = floor_count(1.0);
    let _ = ComponentState::Stub { kind: 0, active: false };
    let _ = component_type::BRUSH;
};
