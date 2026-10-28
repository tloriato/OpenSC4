//! The lot (`cSC4Lot`, lr §3) and the business unit (`cSC4BusinessUnit`,
//! lr §4) records.

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use super::{CLSID_BUSINESS_UNIT, CLSID_LOT, IID_BUSINESS_UNIT, IID_LOT};
use crate::property::PropertyHolder;
use crate::stream::{InStream, OutStream, RefValue, SaveObject};
use crate::traffic_simulator::{read_trip, write_trip, TripData};

/// The lot record version written (§3.2 #1); `Read` accepts ≤ 8.
pub const LOT_RECORD_VERSION: u16 = 8;
/// Versions below this use the legacy layout (§3.4 step 3).
pub const LOT_LEGACY_BELOW: u16 = 6;
/// The trip list is present only above this version (§3.4 step 7).
pub const LOT_TRIPS_ABOVE: u16 = 6;
/// The strategy byte is present only above this version (§3.4 step 8).
pub const LOT_STRATEGY_ABOVE: u16 = 7;
/// The business-unit record version (§4 #1).
pub const BUSINESS_UNIT_RECORD_VERSION: u16 = 1;

/// Lot flag bits (§3.2 #3).
pub mod lot_flag {
    pub const TERRAIN_LOCKED: u8 = 1 << 0;
    pub const FOUNDATION: u8 = 1 << 1;
    pub const RETAINING_WALLS: u8 = 1 << 2;
    pub const NEEDS_POWER: u8 = 1 << 3;
    pub const HAS_POWER: u8 = 1 << 4;
    pub const HISTORICAL: u8 = 1 << 5;
    pub const VERTICES_LOCKED: u8 = 1 << 6;
    pub const TRAFFIC_MARK: u8 = 1 << 7;
}

/// A population map (developer type → count), written in ascending key
/// order (§3.2 #21, #22).
pub type PopulationMap = BTreeMap<u32, u16>;

/// The 27 fields of §3.2 as data.
#[derive(Clone, Debug)]
pub struct LotRecord {
    /// #2: the configuration's `GetID()`, 0 = none.
    pub configuration_id: u32,
    /// #3: [`lot_flag`] bits.
    pub flags: u8,
    /// #4: x1, z1, x2, z2 in cells, inclusive.
    pub rect: [u8; 4],
    /// #5: x, z.
    pub location: [u8; 2],
    /// #6: metres.
    pub altitude: f32,
    /// #7: slope stored by `SetAltitude`.
    pub slope_at_altitude: f32,
    /// #8: slope at the last `UpdateSlope`.
    pub slope: f32,
    /// #9: size x, size z.
    pub size: [u8; 2],
    /// #10: 0…3.
    pub facing: u8,
    /// #11: `HabitationState`.
    pub state: u8,
    /// #12.
    pub condition: u8,
    /// #13.
    pub zone_type: u8,
    /// #14: 0…3.
    pub wealth: u8,
    /// #15: Julian day of the last state change.
    pub state_day: u32,
    /// #16: 0 = no building.
    pub building_type: u32,
    /// #17: copied as is, stale values included *[quirk]*.
    pub vacate_reason: u8,
    /// #18: `cSC4BusinessUnit` (read IID `0x6A233069`).
    pub business_unit: RefValue,
    /// #19: another lot (read IID `0xE818FCED`).
    pub travel_designate: RefValue,
    /// #20, #21: `GetPopulations`.
    pub populations: Vec<PopulationMap>,
    /// #22.
    pub travel_populations: PopulationMap,
    /// #23: `GetTravelJobs`.
    pub travel_jobs: [f32; 3],
    /// #24: the plain holder (§3.6).
    pub properties: PropertyHolder,
    /// #25, #26: front to back.
    pub trips: Vec<TripData>,
    /// #27: traffic strategy byte.
    pub strategy: u8,
}

impl Default for LotRecord {
    /// The constructor zeroes every field (§3.1).
    fn default() -> Self {
        LotRecord {
            configuration_id: 0,
            flags: 0,
            rect: [0; 4],
            location: [0; 2],
            altitude: 0.0,
            slope_at_altitude: 0.0,
            slope: 0.0,
            size: [0; 2],
            facing: 0,
            state: 0,
            condition: 0,
            zone_type: 0,
            wealth: 0,
            state_day: 0,
            building_type: 0,
            vacate_reason: 0,
            business_unit: RefValue::Null,
            travel_designate: RefValue::Null,
            populations: Vec::new(),
            travel_populations: PopulationMap::new(),
            travel_jobs: [0.0; 3],
            properties: PropertyHolder::new(),
            trips: Vec::new(),
            strategy: 0,
        }
    }
}

/// Writes a map: u8 count, then the entries in ascending key order.
///
/// A map of more than 255 entries has its count truncated by the u8
/// *[quirk; never observed]* (§3.2). Whether the original then writes every
/// entry or only the counted ones is not stated: the literal reading (the
/// loop runs over the map) writes every entry (question OP11).
fn put_map(out: &mut dyn OutStream, m: &PopulationMap) {
    out.put_u8(m.len() as u8);
    for (&k, &v) in m {
        out.put_u32(k);
        out.put_u16(v);
    }
}

/// Reads a map's entries **into** `m` (`operator[]`: an existing key is
/// overwritten, nothing is cleared; §3.4 step 5).
fn get_map_into(inp: &mut dyn InStream, m: &mut PopulationMap) -> Option<()> {
    let n = inp.get_u8()?;
    for _ in 0..n {
        let k = inp.get_u32()?;
        let v = inp.get_u16()?;
        m.insert(k, v);
    }
    Some(())
}

impl LotRecord {
    /// Decode the persisted fields without live `Init` or configuration lookup.
    /// Unlike `Lot::write`, `LotRecord::write` saves the configuration ID here.
    pub fn read(&mut self, inp: &mut dyn InStream) -> bool {
        let Some(v) = inp.get_u16() else { return false };
        if v > LOT_RECORD_VERSION {
            return false;
        }
        match self.read_fields(inp, v) {
            Err(()) => false,
            Ok(_) => inp.error() == 0,
        }
    }

    /// `Write` (`0x006C0080`, §3.2): version 8 always. A trip write failure
    /// (a stream error) returns false without writing field 27.
    pub fn write(&self, out: &mut dyn OutStream) -> bool {
        out.put_u16(LOT_RECORD_VERSION);
        out.put_u32(self.configuration_id);
        out.put_u8(self.flags);
        for &b in &self.rect {
            out.put_u8(b);
        }
        out.put_u8(self.location[0]);
        out.put_u8(self.location[1]);
        out.put_f32(self.altitude);
        out.put_f32(self.slope_at_altitude);
        out.put_f32(self.slope);
        out.put_u8(self.size[0]);
        out.put_u8(self.size[1]);
        out.put_u8(self.facing);
        out.put_u8(self.state);
        out.put_u8(self.condition);
        out.put_u8(self.zone_type);
        out.put_u8(self.wealth);
        out.put_u32(self.state_day);
        out.put_u32(self.building_type);
        out.put_u8(self.vacate_reason);
        out.put_ref(&self.business_unit);
        out.put_ref(&self.travel_designate);
        // u8 count (truncating, §3.2 quirk), then each map.
        out.put_u8(self.populations.len() as u8);
        for m in &self.populations {
            put_map(out, m);
        }
        put_map(out, &self.travel_populations);
        for &j in &self.travel_jobs {
            out.put_f32(j);
        }
        self.properties.write(out);
        out.put_u32(self.trips.len() as u32);
        for t in &self.trips {
            if !write_trip(out, t) {
                return false;
            }
        }
        out.put_u8(self.strategy);
        out.error() == 0
    }

    /// The field reads of §3.4 steps 3–8 after the version (`v` ≤ 8).
    /// `Err(())` = a trip `Read` failed (return false at once); `Ok(None)` =
    /// the stream ran out (the result is then `GetError() == 0`).
    fn read_fields(&mut self, inp: &mut dyn InStream, v: u16) -> Result<Option<()>, ()> {
        macro_rules! g {
            ($e:expr) => {
                match $e {
                    Some(x) => x,
                    None => return Ok(None),
                }
            };
        }
        let legacy = v < LOT_LEGACY_BELOW;
        self.configuration_id = g!(inp.get_u32());
        self.flags = g!(inp.get_u8());
        // Legacy (< 6): rect, location and size are s32; stored into the u8
        // fields by truncation (literal reading: the live fields are bytes).
        if legacy {
            for i in 0..4 {
                self.rect[i] = g!(inp.get_i32()) as u8;
            }
            self.location = [g!(inp.get_i32()) as u8, g!(inp.get_i32()) as u8];
        } else {
            for i in 0..4 {
                self.rect[i] = g!(inp.get_u8());
            }
            self.location = [g!(inp.get_u8()), g!(inp.get_u8())];
        }
        self.altitude = g!(inp.get_f32());
        self.slope_at_altitude = g!(inp.get_f32());
        self.slope = g!(inp.get_f32());
        if legacy {
            self.size = [g!(inp.get_i32()) as u8, g!(inp.get_i32()) as u8];
        } else {
            self.size = [g!(inp.get_u8()), g!(inp.get_u8())];
        }
        self.facing = g!(inp.get_u8());
        self.state = g!(inp.get_u8());
        self.condition = g!(inp.get_u8());
        self.zone_type = g!(inp.get_u8());
        self.wealth = g!(inp.get_u8());
        if legacy {
            // SetVerticesAreLocked (slot 69) = flag bit 6 (§3.2 #3).
            let b = g!(inp.get_bool());
            self.set_flag(lot_flag::VERTICES_LOCKED, b);
        }
        self.state_day = g!(inp.get_u32());
        self.building_type = g!(inp.get_u32());
        if legacy {
            // Needs power (bit 3), SetHasPower (bit 4), SetHistorical (bit
            // 5): applied as plain flag bits; side effects of the live
            // setters are not part of this record (no legacy save exists).
            let needs = g!(inp.get_bool());
            let has = g!(inp.get_bool());
            let hist = g!(inp.get_bool());
            self.set_flag(lot_flag::NEEDS_POWER, needs);
            self.set_flag(lot_flag::HAS_POWER, has);
            self.set_flag(lot_flag::HISTORICAL, hist);
        }
        self.vacate_reason = g!(inp.get_u8());
        // Step 4: clear / release the old references, then read.
        self.business_unit = RefValue::Null;
        self.business_unit = g!(inp.get_ref_as(IID_BUSINESS_UNIT));
        self.travel_designate = RefValue::Null;
        self.travel_designate = g!(inp.get_ref_as(IID_LOT));
        // Step 5: resize to P, insert into each map; travel map likewise.
        let p = g!(inp.get_u8()) as usize;
        self.populations.resize(p, PopulationMap::new());
        for m in &mut self.populations {
            g!(get_map_into(inp, m));
        }
        g!(get_map_into(inp, &mut self.travel_populations));
        for j in &mut self.travel_jobs {
            *j = g!(inp.get_f32());
        }
        // Step 6: the holder's result is ignored.
        let _ = self.properties.read(inp);
        // Step 7: trips appended to the existing list.
        if v > LOT_TRIPS_ABOVE {
            let t = g!(inp.get_u32());
            for _ in 0..t {
                let mut trip = TripData::default();
                if !read_trip(inp, &mut trip) {
                    return Err(());
                }
                self.trips.push(trip);
            }
        }
        // Step 8.
        if v > LOT_STRATEGY_ABOVE {
            self.strategy = g!(inp.get_u8());
        }
        Ok(Some(()))
    }

    fn set_flag(&mut self, bit: u8, on: bool) {
        if on {
            self.flags |= bit;
        } else {
            self.flags &= !bit;
        }
    }
}

/// A lot in the class-object store (§3). `initialised` is the unsaved flag
/// `Read` sets before checking the version (§3.4 step 1, T9).
#[derive(Debug, Default)]
pub struct Lot {
    pub rec: RefCell<LotRecord>,
    pub initialised: Cell<bool>,
    /// The configuration object `Init` looked up (§3.5 step 3), held as its
    /// ID (`GetID`, configuration slot 3). Not saved; `Write` saves its ID.
    pub configuration: Cell<Option<u32>>,
}

impl Lot {
    pub fn new(rec: LotRecord) -> Rc<Lot> {
        Rc::new(Lot {
            rec: RefCell::new(rec),
            initialised: Cell::new(false),
            configuration: Cell::new(None),
        })
    }

    /// A lot as the constructor leaves it: every field zero except the
    /// strategy byte `0xFF` that `Init` replaces with one draw (§3.5 step 2).
    pub fn constructed() -> Rc<Lot> {
        Lot::new(LotRecord {
            strategy: super::lifecycle::STRATEGY_UNSET,
            ..LotRecord::default()
        })
    }
}

impl SaveObject for Lot {
    fn class_id(&self) -> u32 {
        CLSID_LOT
    }
    /// Field 2 is `object ? object.GetID() : 0` (`0x006C0093`); the ID read
    /// (`+0x30`) is never written back, so a lot read but never `Init`ed
    /// saves 0 whatever it read (§3.2, §3.5, LR4).
    fn write(&self, out: &mut dyn OutStream) -> bool {
        let r = self.rec.borrow();
        let id = self.configuration.get().unwrap_or(0);
        if r.configuration_id == id {
            return r.write(out);
        }
        let mut c = r.clone();
        c.configuration_id = id;
        c.write(out)
    }
    /// `Read` (`0x006C35F0`, §3.4).
    fn read(&self, inp: &mut dyn InStream) -> bool {
        self.initialised.set(true);
        self.rec.borrow_mut().read(inp)
    }
    fn as_any_rc(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }
}

/// The business-unit record (§4).
#[derive(Clone, Debug, Default)]
pub struct BusinessUnitRecord {
    /// Member lots (read IID `0xE818FCED`), in list order.
    pub lots: Vec<RefValue>,
}

impl BusinessUnitRecord {
    pub fn write(&self, out: &mut dyn OutStream) -> bool {
        out.put_u16(BUSINESS_UNIT_RECORD_VERSION);
        out.put_u32(self.lots.len() as u32);
        for l in &self.lots {
            out.put_ref(l);
        }
        out.error() == 0
    }

    /// §4: version 1 only; each lot is **appended** to the member list.
    pub fn read(&mut self, inp: &mut dyn InStream) -> bool {
        let Some(v) = inp.get_u16() else { return false };
        if v != BUSINESS_UNIT_RECORD_VERSION {
            return false;
        }
        let _ = (|| -> Option<()> {
            let n = inp.get_u32()?;
            for _ in 0..n {
                let l = inp.get_ref_as(IID_LOT)?;
                self.lots.push(l);
            }
            Some(())
        })();
        inp.error() == 0
    }
}

/// A business unit in the class-object store (§4).
#[derive(Debug, Default)]
pub struct BusinessUnit {
    pub rec: RefCell<BusinessUnitRecord>,
}

impl SaveObject for BusinessUnit {
    fn class_id(&self) -> u32 {
        CLSID_BUSINESS_UNIT
    }
    fn write(&self, out: &mut dyn OutStream) -> bool {
        self.rec.borrow().write(out)
    }
    fn read(&self, inp: &mut dyn InStream) -> bool {
        self.rec.borrow_mut().read(inp)
    }
    fn as_any_rc(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }
}
