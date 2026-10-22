//! Lot mutation, lot-records §3.2.1–3.2.2 (LR10/LR11).
//!
//! This is a value-model component, not a native lot or an installation path.
//! In particular, `LotRecord`'s property values are not native property-object
//! references. The adapter must supply the copy/message ownership operations
//! below; cloning a `LotRecord` is not an implementation of the native copy.
//! No `RefCell` guard may be held across the synchronous host calls by a native
//! binding: getters can run concurrently and collaborators can read this lot.

use std::collections::BTreeMap;

use super::lot::{lot_flag, LotRecord, PopulationMap};
use crate::x87::r24;

pub const MSG_LOT_STATE_CHANGED: u32 = 0xE8BD_A2E7;

/// BDS slot 39's nonnull profile, including its default for an unknown type.
#[derive(Clone, Debug, Default)]
pub struct BuildingProfile {
    pub purpose: u32,
    pub native_wealth: i32,
    pub construction_resources: u16,
    pub mixed_occupancy: bool,
    pub capacities: Vec<PopulationMap>,
}

/// Native-undefined/faulting container shapes are explicit refusals here,
/// never repaired by growing populations or skipping an empty capacity map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MutationError {
    MissingPopulationMap { index: usize, len: usize },
    EmptyCapacityMap { index: usize },
    CapacityMeanOverflow { index: usize },
}

/// Synchronous collaborators, in call order. All tokens are adapter-defined.
/// The native adapter must preserve common-owner views and reference counts;
/// these callbacks are not permission to forward native methods on Rust data.
pub trait MutationHost {
    type Copy;
    type Message;

    /// Lot slot 46, including the configuration fallback when requested.
    fn building_type(&mut self, lot: &LotRecord, fallback: bool) -> u32;
    /// BDS slot 39; unknown IDs insert/return the default profile.
    fn profile(&mut self, building_type: u32) -> BuildingProfile;
    /// BDS slot 30 (purpose, signed wealth).
    fn developer_type(&mut self, purpose: u32, wealth: i32) -> u32;
    fn city_day(&mut self, lot: &LotRecord) -> u32;
    /// BDS slot 51, even when resources == 0: counter slot 17 then slot 30.
    fn release_construction_resources(&mut self, resources: u16);

    /// Lot slot 48, borrowed building token (0 means no building).
    fn building(&mut self, lot: &LotRecord) -> u64;
    /// BDS slot 33, borrowed developer token (0 means absent).
    fn developer(&mut self, purpose: u32, wealth: i32) -> u64;
    /// Lot slot 9, borrowed configuration token (0 means absent).
    fn configuration(&mut self, lot: &LotRecord) -> u64;
    fn developer_stage(&mut self, developer: u64) -> u32;
    fn configuration_stage(&mut self, configuration: u64) -> u32;
    /// Building slot 3 then result slot 6, borrowed holder.
    fn building_properties(&mut self, building: u64) -> u64;
    /// Holder slot 9 then value slot 72; None means absent property.
    fn property_bool(&mut self, holder: u64, id: u32) -> Option<bool>;

    /// Independent native copy, initially count 0: copy population/travel
    /// maps and scalars; empty trips/runtime tree and null unit/designate;
    /// acquire configuration and each shared nonnull property exactly once.
    fn copy_lot(&mut self, lot: &LotRecord) -> Self::Copy;
    fn retain_copy(&mut self, copy: &Self::Copy);
    /// Create a caller-owned message of MSG_LOT_STATE_CHANGED and assign its
    /// IGZUnknown to copy, acquiring another copy reference (not a transfer).
    fn state_message(&mut self, data: [u32; 4], copy: &Self::Copy) -> Self::Message;
    /// MessagePost(message, false): queue acquires the message's reference.
    fn post_queued(&mut self, message: &Self::Message);
    fn release_message(&mut self, message: Self::Message);
    fn release_copy(&mut self, copy: Self::Copy);
}

fn population_map(lot: &LotRecord, index: usize) -> Result<&PopulationMap, MutationError> {
    lot.populations.get(index).ok_or(MutationError::MissingPopulationMap {
        index,
        len: lot.populations.len(),
    })
}

/// Lot slot 50. The true argument bypasses the state gate, not just state 4.
pub fn capacity(lot: &LotRecord, dev_type: u32, bypass_state: bool, host: &mut impl MutationHost) -> u16 {
    if !bypass_state && lot.state != 3 {
        return 0;
    }
    let building_type = host.building_type(lot, true);
    let profile = host.profile(building_type);
    profile
        .capacities
        .iter()
        .find_map(|c| c.get(&dev_type))
        .copied()
        .unwrap_or(0)
}

/// Lot slot 52. Arithmetic uses foundations §18's per-operation 24-bit
/// precision, including quotients that have no intervening float store.
pub fn available_capacity(
    lot: &LotRecord,
    dev_type: u32,
    include_vacant: bool,
    host: &mut impl MutationHost,
) -> Result<u16, MutationError> {
    if lot.state != 3 && !(include_vacant && lot.state == 4) {
        return Ok(0);
    }
    let building_type = host.building_type(lot, true);
    let profile = host.profile(building_type);
    let Some((index, c)) = profile
        .capacities
        .iter()
        .enumerate()
        .find(|(_, c)| c.contains_key(&dev_type))
    else {
        return Ok(0);
    };
    let p = population_map(lot, index)?;
    let mut fraction = 1.0;
    for (&q, &count) in c {
        if q != dev_type {
            if let Some(&population) = p.get(&q) {
                fraction = r24(fraction - r24(f64::from(population) / f64::from(count)));
            }
        }
    }
    let requested_capacity = c[&dev_type];
    if fraction == 1.0 {
        return Ok(requested_capacity);
    }
    if !profile.mixed_occupancy {
        return Ok(0);
    }
    let product = r24(f64::from(requested_capacity) * fraction);
    let rounded = r24(product + 12_582_912.0) as f32;
    Ok(rounded.to_bits().wrapping_sub(0x4B40_0000) as u16)
}

/// Lot slot 62. Indexing P inserts every capacity key before any occupancy
/// divisions; duplicates contribute in vector/unsigned-key order.
pub fn occupancy_by_type(
    lot: &mut LotRecord,
    host: &mut impl MutationHost,
) -> Result<BTreeMap<u32, f32>, MutationError> {
    let building_type = host.building_type(lot, true);
    let profile = host.profile(building_type);
    let mut means = Vec::with_capacity(profile.capacities.len());
    let mut sum = 0i32;
    for (index, c) in profile.capacities.iter().enumerate() {
        let len = lot.populations.len();
        let p = lot
            .populations
            .get_mut(index)
            .ok_or(MutationError::MissingPopulationMap { index, len })?;
        let mut total = 0i32;
        for (&q, &count) in c {
            p.entry(q).or_insert(0);
            total = total.wrapping_add(i32::from(count));
        }
        if c.is_empty() {
            return Err(MutationError::EmptyCapacityMap { index });
        }
        let mean = total
            .checked_div(c.len() as i32)
            .ok_or(MutationError::CapacityMeanOverflow { index })?;
        sum = sum.wrapping_add(mean);
        means.push(mean);
    }
    let mut occupancy = BTreeMap::new();
    if sum <= 0 {
        return Ok(occupancy);
    }
    let sum = f64::from(sum as f32);
    for (index, c) in profile.capacities.iter().enumerate() {
        let weight = r24(f64::from(means[index]) / sum);
        for (&q, &count) in c {
            let fraction = r24(f64::from(lot.populations[index][&q]) / f64::from(count));
            let contribution = r24(fraction * weight) as f32;
            occupancy
                .entry(q)
                .and_modify(|v| *v += contribution)
                .or_insert(contribution);
        }
    }
    Ok(occupancy)
}

pub fn update_wealth(lot: &mut LotRecord, host: &mut impl MutationHost) -> Result<(), MutationError> {
    let building_type = host.building_type(lot, false);
    if building_type == 0 {
        lot.wealth = 0;
        return Ok(());
    }
    let profile = host.profile(building_type);
    if profile.purpose == 0 {
        return Ok(());
    }
    let occupancy = occupancy_by_type(lot, host)?;
    if lot.wealth == 0 || !occupancy.contains_key(&host.developer_type(profile.purpose, i32::from(lot.wealth))) {
        lot.wealth = profile.native_wealth as u8;
    }
    let mut best = 0.0f32;
    for wealth in (1..=profile.native_wealth).rev() {
        let dev_type = host.developer_type(profile.purpose, wealth);
        if let Some(&value) = occupancy.get(&dev_type) {
            if value > best {
                best = value;
                lot.wealth = wealth as u8;
            }
        }
    }
    Ok(())
}

/// Slot 55's AL result only. No map append, zero removal or unchanged-value
/// shortcut. A refusal is separate from the native no-match AL=0 result.
pub fn set_population(
    lot: &mut LotRecord,
    dev_type: u32,
    population: u16,
    host: &mut impl MutationHost,
) -> Result<bool, MutationError> {
    let clamped = population.min(available_capacity(lot, dev_type, false, host)?);
    let building_type = host.building_type(lot, true);
    let profile = host.profile(building_type);
    let Some(index) = profile.capacities.iter().position(|c| c.contains_key(&dev_type)) else {
        return Ok(false);
    };
    let len = lot.populations.len();
    let p = lot
        .populations
        .get_mut(index)
        .ok_or(MutationError::MissingPopulationMap { index, len })?;
    p.insert(dev_type, clamped);
    update_wealth(lot, host)?;
    Ok(true)
}

pub fn update_needs_power(lot: &mut LotRecord, host: &mut impl MutationHost) {
    let mut needs_power = false;
    if matches!(lot.state, 3 | 4) {
        let building_type = host.building_type(lot, false);
        if building_type != 0 {
            let building = host.building(lot);
            let profile = host.profile(building_type);
            let eligible = if profile.purpose == 0 {
                true
            } else {
                let developer = host.developer(profile.purpose, profile.native_wealth);
                let configuration = host.configuration(lot);
                if developer == 0 || configuration == 0 {
                    false
                } else {
                    let developer_stage = host.developer_stage(developer) as u8;
                    (host.configuration_stage(configuration) as u8) > developer_stage
                }
            };
            if eligible {
                needs_power = if building == 0 {
                    true
                } else {
                    let holder = host.building_properties(building);
                    let suppressed = host.property_bool(holder, 0x2781_2852).unwrap_or(false);
                    let consumes = host.property_bool(holder, 0x2781_2854).unwrap_or(false);
                    consumes && !suppressed
                };
            }
        }
    }
    lot.flags = (lot.flags & !lot_flag::NEEDS_POWER) | if needs_power { lot_flag::NEEDS_POWER } else { 0 };
}

/// Slot 34 deliberately has no success boolean. Host observations receive the
/// already-mutated record. Errors preserve preceding mutations, with no copy,
/// post, fallback, rollback or invented successful container result.
pub fn set_state(
    lot: &mut LotRecord,
    state: u32,
    wealth: u32,
    reason: u32,
    host: &mut impl MutationHost,
) -> Result<(), MutationError> {
    if u32::from(lot.state) == state {
        return Ok(());
    }
    let old = lot.state;
    lot.state = state as u8;
    lot.state_day = host.city_day(lot);
    if old == 2 && lot.building_type != 0 {
        let profile = host.profile(lot.building_type);
        if profile.purpose != 0 {
            host.release_construction_resources(profile.construction_resources);
            if lot.state == 3 {
                let dev_type = host.developer_type(profile.purpose, profile.native_wealth);
                let count = capacity(lot, dev_type, true, host);
                set_population(lot, dev_type, count, host)?;
            }
        }
    }
    if lot.state == 2 {
        let building_type = host.building_type(lot, true);
        if building_type != 0 {
            let profile = host.profile(building_type);
            lot.populations.clear();
            for c in profile.capacities {
                lot.populations.push(c.into_keys().map(|q| (q, 0)).collect());
            }
        }
    } else if lot.state == 4 {
        lot.vacate_reason = reason as u8;
    }
    if !matches!(lot.state, 2 | 3) {
        for p in &mut lot.populations {
            for count in p.values_mut() {
                *count = 0;
            }
        }
    }
    let pre_wealth = lot.wealth;
    update_wealth(lot, host)?;
    let mut message_wealth = lot.wealth;
    if matches!(old, 2 | 3) != matches!(lot.state, 2 | 3) {
        if matches!(old, 2 | 3) {
            message_wealth = pre_wealth;
        } else if wealth != 0 {
            lot.wealth = wealth as u8;
            message_wealth = lot.wealth;
        }
    }
    update_needs_power(lot, host);
    let copy = host.copy_lot(lot);
    host.retain_copy(&copy);
    let message = host.state_message(
        [
            u32::from(old),
            u32::from(lot.state),
            lot.building_type,
            u32::from(message_wealth),
        ],
        &copy,
    );
    host.post_queued(&message);
    host.release_message(message);
    host.release_copy(copy);
    Ok(())
}
