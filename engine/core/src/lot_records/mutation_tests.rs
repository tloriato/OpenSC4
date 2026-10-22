//! Spec fixtures, not native-original comparisons or runtime ownership proof.
use super::lot::{lot_flag, LotRecord, PopulationMap};
use super::mutation::*;

const K: u32 = 0x1010;
const J: u32 = 0x1020;
const B: u32 = 0x1234;

fn map(entries: &[(u32, u16)]) -> PopulationMap {
    entries.iter().copied().collect()
}

#[derive(Default)]
struct Host {
    profile: BuildingProfile,
    fallback: u32,
    calls: Vec<&'static str>,
    copy_input: Option<LotRecord>,
    data: Option<[u32; 4]>,
    building: u64,
    developer: u64,
    configuration: u64,
    stages: [u32; 2],
    properties: [Option<bool>; 2],
    released_resources: Vec<u16>,
    day_observation: Option<(u8, u32)>,
}

impl MutationHost for Host {
    type Copy = ();
    type Message = ();

    fn building_type(&mut self, lot: &LotRecord, fallback: bool) -> u32 {
        self.calls.push(if fallback { "type-true" } else { "type-false" });
        if fallback && lot.building_type == 0 {
            self.fallback
        } else {
            lot.building_type
        }
    }
    fn profile(&mut self, _: u32) -> BuildingProfile {
        self.calls.push("profile");
        self.profile.clone()
    }
    fn developer_type(&mut self, _: u32, wealth: i32) -> u32 {
        self.calls.push("developer-type");
        match wealth {
            1 => K,
            2 => J,
            _ => 0,
        }
    }
    fn city_day(&mut self, lot: &LotRecord) -> u32 {
        self.calls.push("day");
        self.day_observation = Some((lot.state, lot.state_day));
        900
    }
    fn release_construction_resources(&mut self, count: u16) {
        self.calls.push("resources");
        self.released_resources.push(count);
    }
    fn building(&mut self, _: &LotRecord) -> u64 {
        self.calls.push("building");
        self.building
    }
    fn developer(&mut self, _: u32, _: i32) -> u64 {
        self.calls.push("developer");
        self.developer
    }
    fn configuration(&mut self, _: &LotRecord) -> u64 {
        self.calls.push("configuration");
        self.configuration
    }
    fn developer_stage(&mut self, _: u64) -> u32 {
        self.calls.push("developer-stage");
        self.stages[0]
    }
    fn configuration_stage(&mut self, _: u64) -> u32 {
        self.calls.push("configuration-stage");
        self.stages[1]
    }
    fn building_properties(&mut self, _: u64) -> u64 {
        self.calls.push("holder");
        7
    }
    fn property_bool(&mut self, _: u64, id: u32) -> Option<bool> {
        let index = usize::from(id == 0x2781_2854);
        self.calls.push(if index == 0 { "suppress" } else { "consume" });
        self.properties[index]
    }
    fn copy_lot(&mut self, lot: &LotRecord) {
        self.calls.push("copy");
        // Capture the input only. This is not a native snapshot implementation.
        self.copy_input = Some(lot.clone());
    }
    fn retain_copy(&mut self, _: &()) {
        self.calls.push("retain-copy");
    }
    fn state_message(&mut self, data: [u32; 4], _: &()) {
        self.calls.push("message");
        self.data = Some(data);
    }
    fn post_queued(&mut self, _: &()) {
        self.calls.push("post");
    }
    fn release_message(&mut self, _: ()) {
        self.calls.push("release-message");
    }
    fn release_copy(&mut self, _: ()) {
        self.calls.push("release-copy");
    }
}

fn residence(capacities: Vec<PopulationMap>) -> Host {
    Host {
        profile: BuildingProfile {
            purpose: 1,
            native_wealth: 2,
            capacities,
            ..Default::default()
        },
        ..Default::default()
    }
}

#[test]
fn population_first_match_unsigned_keys_no_duplicate_update() {
    let key = 0xFFFF_FFFF;
    let mut h = Host {
        profile: BuildingProfile {
            capacities: vec![map(&[(key, 40)]), map(&[(key, 99)])],
            ..Default::default()
        },
        ..Default::default()
    };
    let mut lot = LotRecord {
        state: 3,
        building_type: B,
        wealth: 2,
        populations: vec![map(&[]), map(&[(key, 7)])],
        ..Default::default()
    };
    assert_eq!(set_population(&mut lot, key, 60, &mut h), Ok(true));
    assert_eq!(lot.populations, vec![map(&[(key, 40)]), map(&[(key, 7)])]);
    assert_eq!(lot.wealth, 2); // purpose-zero profile leaves it unchanged
    assert_eq!(
        h.calls,
        ["type-true", "profile", "type-true", "profile", "type-false", "profile"]
    );
}

#[test]
fn population_zero_unchanged_and_no_match_paths() {
    let mut h = residence(vec![map(&[(K, 100)])]);
    let mut lot = LotRecord {
        state: 1,
        wealth: 2,
        populations: vec![map(&[(K, 7)])],
        ..Default::default()
    };
    h.fallback = B;
    assert_eq!(set_population(&mut lot, K, 99, &mut h), Ok(true));
    assert_eq!(lot.populations[0][&K], 0);
    assert_eq!(lot.wealth, 0); // no stored type despite a fallback type
    lot.wealth = 2;
    assert_eq!(set_population(&mut lot, K, 0, &mut h), Ok(true));
    assert_eq!(lot.wealth, 0); // unchanged assignment still recomputes
    lot.wealth = 3;
    let before = lot.populations.clone();
    assert_eq!(set_population(&mut lot, J, 50, &mut h), Ok(false));
    assert_eq!(lot.wealth, 3);
    assert_eq!(lot.populations, before);
    assert!(h.data.is_none());
}

#[test]
fn mixed_capacity_clamp_and_unrepaired_shapes() {
    let mut h = residence(vec![map(&[(K, 100), (J, 100)])]);
    h.profile.mixed_occupancy = true;
    let mut lot = LotRecord {
        state: 3,
        building_type: B,
        populations: vec![map(&[(K, 0), (J, 25)])],
        ..Default::default()
    };
    assert_eq!(set_population(&mut lot, K, 90, &mut h), Ok(true));
    assert_eq!(lot.populations[0][&K], 75);
    h.profile.mixed_occupancy = false;
    assert_eq!(set_population(&mut lot, K, 90, &mut h), Ok(true));
    assert_eq!(lot.populations[0][&K], 0);
    lot.populations.clear();
    assert_eq!(
        set_population(&mut lot, K, 1, &mut h),
        Err(MutationError::MissingPopulationMap { index: 0, len: 0 })
    );
    assert!(lot.populations.is_empty());
}

#[test]
fn wealth_inserts_capacity_keys_and_keeps_supported_zero_candidate() {
    let mut h = residence(vec![map(&[(K, 100), (J, 100)])]);
    let mut lot = LotRecord {
        building_type: B,
        wealth: 1,
        populations: vec![map(&[])],
        ..Default::default()
    };
    update_wealth(&mut lot, &mut h).unwrap();
    assert_eq!(lot.populations, vec![map(&[(K, 0), (J, 0)])]);
    assert_eq!(lot.wealth, 1);
    for wealth in [0, 3] {
        lot.wealth = wealth;
        update_wealth(&mut lot, &mut h).unwrap();
        assert_eq!(lot.wealth, 2);
    }
    lot.populations[0] = map(&[(K, 50), (J, 50)]);
    lot.wealth = 1;
    update_wealth(&mut lot, &mut h).unwrap();
    assert_eq!(lot.wealth, 2); // positive tie: descending wealth wins
    lot.populations[0] = map(&[(K, 75), (J, 25)]);
    update_wealth(&mut lot, &mut h).unwrap();
    assert_eq!(lot.wealth, 1);
}

#[test]
fn occupancy_accumulates_duplicates_and_preserves_nonfinite_values() {
    let mut h = residence(vec![map(&[(K, 100)]), map(&[(K, 300), (J, 300)])]);
    let mut lot = LotRecord {
        building_type: B,
        populations: vec![map(&[(K, 100)]), map(&[(K, 0), (J, 300)])],
        ..Default::default()
    };
    assert_eq!(
        occupancy_by_type(&mut lot, &mut h).unwrap(),
        [(K, 0.25), (J, 0.75)].into()
    );
    h.profile.capacities = vec![map(&[(K, 0), (J, 100)])];
    lot.populations = vec![map(&[(K, 0), (J, 1)])];
    assert!(occupancy_by_type(&mut lot, &mut h).unwrap()[&K].is_nan());
}

#[test]
fn empty_vector_is_valid_empty_map_is_refusal_after_state_day() {
    let mut h = residence(vec![]);
    let mut lot = LotRecord {
        state: 1,
        building_type: B,
        wealth: 3,
        ..Default::default()
    };
    update_wealth(&mut lot, &mut h).unwrap();
    assert_eq!(lot.wealth, 2);
    h.profile.capacities.push(map(&[]));
    assert_eq!(
        set_state(&mut lot, 2, 0, 0, &mut h),
        Err(MutationError::EmptyCapacityMap { index: 0 })
    );
    assert_eq!((lot.state, lot.state_day), (2, 900));
    assert!(h.copy_input.is_none());
    assert!(h.data.is_none());
}

#[test]
fn equal_whole_state_noop_but_same_low_byte_changes_day_and_posts() {
    let mut h = Host::default();
    let mut lot = LotRecord {
        state: 1,
        wealth: 2,
        state_day: 11,
        populations: vec![map(&[(K, 7)])],
        ..Default::default()
    };
    set_state(&mut lot, 1, 3, 6, &mut h).unwrap();
    assert!(h.calls.is_empty());
    set_state(&mut lot, 0x101, 0, 0, &mut h).unwrap();
    assert_eq!(h.day_observation, Some((1, 11)));
    assert_eq!((lot.state, lot.state_day, lot.wealth), (1, 900, 0));
    assert_eq!(lot.populations[0][&K], 0);
    assert_eq!(h.data, Some([1, 1, 0, 0]));
    assert_eq!(
        &h.calls[h.calls.len() - 6..],
        [
            "copy",
            "retain-copy",
            "message",
            "post",
            "release-message",
            "release-copy"
        ]
    );
}

#[test]
fn construction_fill_nested_wealth_ignores_within_group_override() {
    let mut h = residence(vec![map(&[(K, 100), (J, 100)])]);
    h.profile.construction_resources = 9;
    let mut lot = LotRecord {
        state: 2,
        building_type: B,
        wealth: 1,
        populations: vec![map(&[(K, 0), (J, 0)])],
        ..Default::default()
    };
    set_state(&mut lot, 3, 1, 6, &mut h).unwrap();
    assert_eq!(h.released_resources, [9]);
    assert_eq!(lot.populations, vec![map(&[(K, 0), (J, 100)])]);
    assert_eq!(lot.wealth, 2);
    assert_eq!(h.data, Some([2, 3, B, 2]));
    assert_eq!(&h.calls[..3], ["day", "profile", "resources"]);
}

#[test]
fn leaving_uses_precomputed_wealth_and_stale_reason_rules() {
    let mut h = Host::default();
    let mut lot = LotRecord {
        state: 3,
        wealth: 2,
        flags: lot_flag::NEEDS_POWER | lot_flag::HAS_POWER,
        populations: vec![map(&[(K, 7)])],
        travel_populations: map(&[(K, 8)]),
        travel_jobs: [1.0, 2.0, 3.0],
        ..Default::default()
    };
    set_state(&mut lot, 4, 3, 0x106, &mut h).unwrap();
    let snapshot_input = h.copy_input.as_ref().unwrap();
    assert_eq!((lot.wealth, lot.vacate_reason), (0, 6));
    assert_eq!(h.data, Some([3, 4, 0, 2]));
    assert_eq!(snapshot_input.wealth, 0);
    assert_eq!(snapshot_input.flags, lot_flag::HAS_POWER);
    assert_eq!(snapshot_input.populations[0][&K], 0);
    assert_eq!(snapshot_input.travel_populations[&K], 8);
    assert_eq!(snapshot_input.travel_jobs, [1.0, 2.0, 3.0]);
    set_state(&mut lot, 1, 0, 99, &mut h).unwrap();
    assert_eq!(lot.vacate_reason, 6);
}

#[test]
fn entering_whole_wealth_nonzero_low_byte_zero_and_unknown_profile() {
    let mut h = Host::default(); // nonzero unknown type: purpose-zero default
    let mut lot = LotRecord {
        state: 4,
        building_type: B,
        wealth: 1,
        ..Default::default()
    };
    set_state(&mut lot, 3, 0x100, 0, &mut h).unwrap();
    assert_eq!(lot.wealth, 0);
    assert_eq!(h.data, Some([4, 3, B, 0]));
    lot.state = 4;
    lot.wealth = 1;
    set_state(&mut lot, 3, 0, 0, &mut h).unwrap();
    assert_eq!(lot.wealth, 1);
    assert_eq!(h.data, Some([4, 3, B, 1]));
}

#[test]
fn zero_resources_still_released_without_fill_and_construction_rebuilds() {
    let mut h = residence(vec![map(&[(K, 100)]), map(&[(J, 200)])]);
    let mut lot = LotRecord {
        state: 2,
        building_type: B,
        populations: vec![map(&[(K, 9)])],
        ..Default::default()
    };
    // Outside 2/3 zeroes existing maps; wealth then refuses missing P[1].
    assert_eq!(
        set_state(&mut lot, 4, 0, 6, &mut h),
        Err(MutationError::MissingPopulationMap { index: 1, len: 1 })
    );
    assert_eq!(h.released_resources, [0]);
    assert_eq!(lot.populations[0][&K], 0);
    assert_eq!(lot.vacate_reason, 6);
    assert!(h.data.is_none());
    set_state(&mut lot, 2, 0, 0, &mut h).unwrap();
    assert_eq!(lot.populations, vec![map(&[(K, 0)]), map(&[(J, 0)])]);
}

#[test]
fn power_low_unsigned_bytes_and_both_property_lookups_before_copy() {
    let mut h = residence(vec![]);
    h.developer = 1;
    h.configuration = 2;
    h.stages = [0x101, 0x202];
    h.building = 3;
    h.properties = [Some(true), Some(true)];
    let mut lot = LotRecord {
        state: 3,
        building_type: B,
        ..Default::default()
    };
    update_needs_power(&mut lot, &mut h);
    assert_eq!(lot.flags & lot_flag::NEEDS_POWER, 0);
    assert_eq!(
        h.calls,
        [
            "type-false",
            "building",
            "profile",
            "developer",
            "configuration",
            "developer-stage",
            "configuration-stage",
            "holder",
            "suppress",
            "consume"
        ]
    );
    h.properties[0] = None;
    update_needs_power(&mut lot, &mut h);
    assert_ne!(lot.flags & lot_flag::NEEDS_POWER, 0);
    h.stages = [0xFF, 0x100];
    update_needs_power(&mut lot, &mut h);
    assert_eq!(lot.flags & lot_flag::NEEDS_POWER, 0);
    h.profile.purpose = 0;
    h.building = 0;
    h.calls.clear();
    set_state(&mut lot, 4, 0, 0, &mut h).unwrap();
    assert_ne!(h.copy_input.as_ref().unwrap().flags & lot_flag::NEEDS_POWER, 0);
    assert!(!h.calls.contains(&"developer"));
}

#[test]
fn native_zero_wealth_has_no_forced_minimum() {
    let mut h = residence(vec![map(&[(K, 100)])]);
    h.profile.native_wealth = 0;
    let mut lot = LotRecord {
        building_type: B,
        populations: vec![map(&[(K, 50)])],
        ..Default::default()
    };
    update_wealth(&mut lot, &mut h).unwrap();
    assert_eq!(lot.wealth, 0);
    lot.wealth = 1;
    update_wealth(&mut lot, &mut h).unwrap();
    assert_eq!(lot.wealth, 1);
}
