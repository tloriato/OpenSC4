// SPDX-License-Identifier: GPL-3.0-or-later
//! cSC4RegionalCity, implemented only from the regional-city specifications.
//! State is ordinary owned data; getters never borrow a RefCell. Host services
//! and the ownership of live connections/deals remain behind their interfaces.

mod codec;
pub mod archive;
mod connections;
mod handles;
pub use codec::{
    read_package, read_path, write_package, write_path, CoreElements, Package, ReadContext, RecordElements,
    RecordError, RecordReader, RECORD_KEY, VERSION_KEY,
};
pub use connections::{ConnectionHost, IncomingCity, PendingChange, PreparedChange};
pub use handles::{ConnectionHandle, DealHandle};

use crate::neighbors::{deal_type, flag, CityId, NeighborConnection, NeighborDeal};
use std::collections::BTreeMap;
use std::rc::Rc;

pub const CLASS_ID: u32 = 0x6A03_7C24;
pub const INTERFACE_ID: u32 = 0x69D2_BE1D;
pub const MESSAGES: [u32; 5] = [0x26C6_3342, 0x26C6_3343, 0x26D3_1EC1, 0x26D3_1EC3, 0xCA63_D517];
pub const CONNECTIONS_READY: u32 = 0x6AC2_84F3;
pub const TAX_TYPES: [u32; 12] = [
    0x1010, 0x1020, 0x1030, 0x3110, 0x3120, 0x3130, 0x3320, 0x3330, 0x4100, 0x4200, 0x4300, 0x4400,
];

pub fn answers_iid(iid: u32) -> bool {
    matches!(iid, 1 | INTERFACE_ID | 0x4522_94AA | 0xE4FD_A3D4)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Commodity {
    pub produced: f64,
    pub demanded: f64,
    pub exported: f64,
    pub imported: f64,
    pub cost_per_unit: f64,
}

/// Explicit fields also provide the simple copy-in/copy-out slots of §3.
/// Strings are byte strings, matching the stream's GZStr encoding.
#[derive(Clone, Debug)]
pub struct RegionalCity<C = Rc<NeighborConnection>, D = Rc<NeighborDeal>> {
    pub initialized: bool,
    pub references: u32,
    pub position: (i32, i32),
    pub size: (i32, i32),
    pub population: i32,
    pub commercial_jobs: i32,
    pub industrial_jobs: i32,
    pub workforce: f32,
    pub mayor_rating: i8,
    pub difficulty: u8,
    pub populations: BTreeMap<u32, i32>,
    pub extrapolated: BTreeMap<u32, i32>,
    pub potential: BTreeMap<u32, i32>,
    pub tax_rates: [f32; 12],
    pub connections: Vec<C>,
    pub pending: Vec<PendingChange>,
    pub moved: bool,
    pub serial: u32,
    pub original_locale: (i32, i32),
    pub last_locale: (i32, i32),
    pub birth_date: u32,
    pub file_path: Vec<u8>,
    pub city_name: Vec<u8>,
    pub mayor_name: Vec<u8>,
    pub description: Vec<u8>,
    pub utility_advisor_name: Vec<u8>,
    pub unknown_strings: [Vec<u8>; 3],
    pub world: [f32; 3],
    pub established: bool,
    pub tutorial_id: i32,
    pub budget: f64,
    pub income: f64,
    pub commodities: BTreeMap<i32, Commodity>,
    pub other_deals: Vec<D>,
    pub local_deals: Vec<D>,
    pub points: BTreeMap<u32, Vec<[f32; 3]>>,
}

pub trait Lifecycle {
    fn subscribe(&mut self, message: u32, enable: bool);
}

impl RegionalCity {
    pub fn new(application_default_difficulty: u8) -> Self {
        Self::with_handles(application_default_difficulty)
    }

    pub fn placeholder(
        default_difficulty: u8,
        position: (i32, i32),
        size_exponent: u32,
        name: &[u8],
        host: &mut impl Lifecycle,
    ) -> Self {
        Self::placeholder_with_handles(default_difficulty, position, size_exponent, name, host)
    }
}

impl<C: ConnectionHandle, D: DealHandle> RegionalCity<C, D> {
    pub fn with_handles(application_default_difficulty: u8) -> Self {
        Self {
            initialized: false,
            references: 0,
            position: (0, 0),
            size: (1, 1),
            population: 0,
            commercial_jobs: 0,
            industrial_jobs: 0,
            workforce: 0.5,
            mayor_rating: 0,
            difficulty: application_default_difficulty,
            populations: BTreeMap::new(),
            extrapolated: BTreeMap::new(),
            potential: BTreeMap::new(),
            tax_rates: [7.0; 12],
            connections: Vec::new(),
            pending: Vec::new(),
            moved: false,
            serial: 0,
            original_locale: (0, 0),
            last_locale: (0, 0),
            birth_date: 0,
            file_path: Vec::new(),
            city_name: Vec::new(),
            mayor_name: Vec::new(),
            description: Vec::new(),
            utility_advisor_name: Vec::new(),
            unknown_strings: Default::default(),
            world: [0.0; 3],
            established: false,
            tutorial_id: -1,
            budget: 0.0,
            income: 0.0,
            commodities: (1..=3).map(|c| (c, Commodity::default())).collect(),
            other_deals: Vec::new(),
            local_deals: Vec::new(),
            points: BTreeMap::new(),
        }
    }

    pub fn placeholder_with_handles(
        default_difficulty: u8,
        position: (i32, i32),
        size_exponent: u32,
        name: &[u8],
        host: &mut impl Lifecycle,
    ) -> Self {
        let mut c = Self::with_handles(default_difficulty);
        c.init(host);
        // Literal 2^k size from records §2.5; callers supply a valid tile class.
        let size = 1_i32.wrapping_shl(size_exponent);
        c.size = (size, size);
        c.set_position(position.0, position.1, false);
        c.city_name = name.to_vec();
        c
    }

    pub fn init(&mut self, host: &mut impl Lifecycle) -> bool {
        if !self.initialized {
            for m in MESSAGES {
                host.subscribe(m, true);
            }
            self.initialized = true;
        }
        true
    }

    pub fn shutdown(&mut self, host: &mut impl Lifecycle) -> bool {
        if self.initialized {
            for m in MESSAGES {
                host.subscribe(m, false);
            }
            self.connections.clear();
            self.other_deals.clear();
            self.local_deals.clear();
            self.initialized = false;
        }
        true
    }

    pub fn add_ref(&mut self) -> u32 {
        self.references = self.references.wrapping_add(1);
        self.references
    }

    /// A zero result tells the wrapper to destroy the object.
    pub fn release(&mut self, host: &mut impl Lifecycle) -> u32 {
        if self.references > 1 {
            self.references -= 1;
        } else {
            self.shutdown(host);
            self.references = 0;
        }
        self.references
    }

    pub fn query_interface(&mut self, iid: u32) -> bool {
        if !answers_iid(iid) {
            return false;
        }
        self.add_ref();
        true
    }

    pub fn get_population(&self, t: u32) -> i32 {
        self.populations.get(&t).copied().unwrap_or(0)
    }
    pub fn get_extrapolated_population(&self, t: u32) -> i32 {
        self.extrapolated
            .get(&t)
            .copied()
            .unwrap_or_else(|| self.get_population(t))
    }
    pub fn get_allowable_extrapolation(&mut self, t: u32) -> i32 {
        let p = *self.potential.entry(t).or_default();
        let e = *self.extrapolated.entry(t).or_default();
        p.wrapping_sub(e).max(0)
    }
    pub fn extrapolate_growth(&mut self, t: u32, add: f32) {
        let e = self.extrapolated.entry(t).or_default();
        *e = trunc_i32(*e as f32 + add);
    }
    pub fn reset_growth(&mut self) {
        for (&t, &p) in &self.populations {
            self.potential.insert(
                t,
                if p <= 0 {
                    0
                } else {
                    trunc_i32((1.1_f32 * p as f32).max(1000.0))
                },
            );
            self.extrapolated.insert(t, p);
        }
    }
    pub fn get_tax_rate(&self, t: u32) -> f32 {
        TAX_TYPES
            .iter()
            .position(|&v| v == t)
            .map_or(7.0, |i| self.tax_rates[i])
    }
    pub fn get_workforce_percentage(&self) -> &f32 {
        &self.workforce
    }
    pub fn is_tutorial(&self) -> bool {
        self.tutorial_id != -1
    }
    pub fn world_hemisphere(&self) -> u32 {
        if self.world[1] > 0.0 {
            1
        } else {
            2
        }
    }
    pub fn get_original_language_and_country(&self, out: &mut (i32, i32)) -> bool {
        *out = self.original_locale;
        true
    }
    pub fn get_last_language_and_country(&self, out: &mut (i32, i32)) -> bool {
        *out = self.last_locale;
        true
    }
    pub fn get_points_of_interest(&self, t: u32, out: &mut Vec<[f32; 3]>) {
        if let Some(p) = self.points.get(&t) {
            out.clone_from(p);
        }
    }
    pub fn commodity(&mut self, c: i32) -> &mut Commodity {
        self.commodities.entry(c).or_default()
    }
    pub fn get_commodity_balance(&mut self, c: i32) -> f64 {
        self.get_commodity_balance_with_precision(c, crate::x87::Precision::Bits24)
    }

    pub fn get_commodity_balance_with_precision(&mut self, c: i32, pc: crate::x87::Precision) -> f64 {
        let c = self.commodity(c);
        // Foundations §18.4: live PC, with the first sum stored as double.
        // Loads stay unrounded; getter/order remains produced/imported/demanded/exported.
        pc.round(pc.round(c.produced + c.imported) - pc.round(c.demanded + c.exported))
    }

    pub fn set_position(&mut self, x: i32, z: i32, rearrange: bool) {
        let delta = (x.wrapping_sub(self.position.0), z.wrapping_sub(self.position.1));
        self.position = (x, z);
        if !rearrange {
            return;
        }
        self.moved = true;
        for c in &self.connections {
            c.repair(self.cell_counts(), self.position, self.size);
        }
        for d in self.other_deals.iter().chain(&self.local_deals) {
            let a = d.city_side();
            let b = d.other_side();
            d.set_city_side(a.0.wrapping_add(delta.0), a.1.wrapping_add(delta.1));
            d.set_other_side(b.0.wrapping_add(delta.0), b.1.wrapping_add(delta.1));
        }
    }
    pub fn cell_counts(&self) -> (i32, i32) {
        (self.size.0.wrapping_mul(64), self.size.1.wrapping_mul(64))
    }
    pub fn find_connection(&self, t: u32, x: i32, z: i32) -> Option<&C> {
        self.connections
            .iter()
            .find(|c| c.conn_type() == t && c.cell() == (x, z))
    }
    pub fn get_all_connections(&self, out: &mut Vec<C>) {
        out.clone_from(&self.connections);
    }
    pub fn get_local_deals(&self, out: &mut Vec<D>) {
        out.clone_from(&self.local_deals);
    }

    pub fn update_local_deals(&mut self, this: CityId, host: &impl DealRegion) -> bool {
        extract(&mut self.other_deals, &mut self.local_deals, true, this, host);
        for d in &self.other_deals {
            d.set_flags(flag::SILENT, false);
        }
        true
    }
    pub fn set_local_deals(&mut self, list: &mut Vec<D>, this: CityId, host: &impl DealRegion) -> bool {
        self.local_deals.clear();
        extract(list, &mut self.local_deals, false, this, host);
        self.update_import_export(this, host);
        true
    }
    pub fn update_import_export(&mut self, this: CityId, host: &impl DealRegion) {
        for c in 1..=3 {
            let c = self.commodity(c);
            c.imported = 0.0;
            c.exported = 0.0;
        }
        for d in self.other_deals.iter().chain(&self.local_deals) {
            let Some(other) = host.city_at(d.other_side()) else {
                continue;
            };
            if d.flags() & 0xBC != 0 {
                continue;
            }
            let Some(c) = deal_type::commodity(d.deal_type()) else {
                continue;
            };
            let export = deal_type::is_buy(d.deal_type()) == (other == this);
            let c = self.commodities.entry(c).or_default();
            if export {
                c.exported += f64::from(d.amount());
            } else {
                c.imported += f64::from(d.amount());
            }
        }
    }
    pub fn return_local_deals(&mut self) {
        while let Some(d) = self.local_deals.pop() {
            self.other_deals.push(d);
        }
    }

    pub fn setup_preferences(&self) -> Preferences {
        Preferences {
            city_name: c_string(&self.city_name),
            mayor_name: c_string(&self.mayor_name),
            difficulty: self.difficulty,
        }
    }
    pub fn update_preferences(&mut self, prefs: &Preferences) {
        self.city_name = c_string(&prefs.city_name);
        self.mayor_name = c_string(&prefs.mayor_name);
        self.difficulty = prefs.difficulty;
        let end = self
            .file_path
            .iter()
            .rposition(|b| matches!(b, b'/' | b'\\' | b':'))
            .map_or(0, |i| i + 1);
        self.file_path.truncate(end);
        self.file_path.extend(&self.city_name);
        self.file_path.extend(b".sc4");
    }
    pub fn setup_city(&self, host: &mut impl CitySetup) {
        host.set_mayor_name(&self.mayor_name);
        host.set_city_name(&self.city_name);
        host.set_difficulty(self.difficulty);
        let path = host.create_save_path(&self.city_name);
        host.set_file_path(&path);
        host.set_size((self.size.0.wrapping_mul(1024), self.size.1.wrapping_mul(1024)));
    }
    pub fn update_city_cache(&mut self, host: &mut impl CacheHost<C, D>) {
        self.mayor_name = host.mayor_name();
        self.city_name = host.city_name();
        self.difficulty = host.difficulty();
        self.file_path = host.file_path();
        self.serial = host.serial();
        self.established = host.established();
        self.mayor_rating = host.mayor_rating();
        for i in 0..12 {
            self.tax_rates[i] = host.tax_rate(i);
        }
        self.connections = host.connections();
        host.update_commodity_stats(self);
        if self.tutorial_id == -1 {
            if let Some(t) = host.tutorial_id() {
                self.tutorial_id = t;
            }
        }
        if let Some((p, w)) = host.residential() {
            self.population = p;
            self.workforce = w;
        }
        self.populations = host.population_summary();
        let mut jobs = |types: &[u32]| {
            types.iter().fold(0_i32, |n, &t| {
                let j = host.jobs_per_unit(t);
                n.wrapping_add(trunc_i32(self.get_population(t) as f32 * ((j[3] + j[2]) + j[1])))
            })
        };
        let commercial = jobs(&[0x3111, 0x3121, 0x3131, 0x3321, 0x3331]);
        let industrial = jobs(&[0x4101, 0x4201, 0x4301, 0x4401]);
        self.commercial_jobs = commercial;
        self.industrial_jobs = industrial;
        self.points.clear();
        for t in 0..=1 {
            self.points.insert(
                t,
                host.lots(t)
                    .into_iter()
                    .filter(|(live, _)| *live)
                    .map(|(_, p)| p)
                    .collect(),
            );
        }
    }
    pub fn do_message(&mut self, message: u32, context: MessageContext, host: &mut impl MessageHost<C, D>) -> bool {
        match message {
            0x26C6_3342 => self.return_local_deals(),
            0x26D3_1EC3 if context.names_current_city => self.return_local_deals(),
            0x26C6_3343 | 0x26D3_1EC1 if context.application_object && context.city_exists => {
                self.update_city_cache(host);
                if message == 0x26C6_3343 {
                    self.reset_growth();
                }
            }
            0xCA63_D517 if context.application_object && context.city_exists => {
                self.reconcile_connections(context.established, context.forced, host);
            }
            _ => {}
        }
        true
    }
}

macro_rules! commodity_slots {
    ($($get:ident, $set:ident, $field:ident);* $(;)?) => {$ (
        impl<C: ConnectionHandle, D: DealHandle> RegionalCity<C, D> {
            pub fn $get(&mut self, c: i32) -> f64 { self.commodity(c).$field }
            pub fn $set(&mut self, c: i32, value: f64) {
                self.commodity(c).$field = if value < 0.0 { 0.0 } else { value };
            }
        }
    )*};
}
commodity_slots! {
    get_produced, set_produced, produced; get_demanded, set_demanded, demanded;
    get_exported, set_exported, exported; get_imported, set_imported, imported;
    get_cost_per_unit, set_cost_per_unit, cost_per_unit;
}

macro_rules! copy_slots {
    ($($get:ident, $set:ident, $field:ident, $ty:ty);* $(;)?) => {$ (
        impl<C: ConnectionHandle, D: DealHandle> RegionalCity<C, D> {
            pub fn $get(&self) -> $ty { self.$field }
            pub fn $set(&mut self, value: $ty) { self.$field = value; }
        }
    )*};
}
copy_slots! {
    get_city_size, set_city_size, size, (i32, i32);
    get_serial_number, set_serial_number, serial, u32;
    get_birth_date, set_birth_date, birth_date, u32;
    get_established, set_established, established, bool;
    get_budget, set_budget, budget, f64;
    get_income, set_income, income, f64;
    get_tutorial_guid, set_tutorial_guid, tutorial_id, i32;
}
macro_rules! string_slots {
    ($($get:ident, $set:ident, $field:ident);* $(;)?) => {$ (
        impl<C: ConnectionHandle, D: DealHandle> RegionalCity<C, D> {
            pub fn $get(&self, out: &mut Vec<u8>) { out.clone_from(&self.$field); }
            pub fn $set(&mut self, value: &[u8]) { self.$field = value.to_vec(); }
        }
    )*};
}
string_slots! {
    get_file_path, set_file_path, file_path;
    get_city_name, set_city_name, city_name;
    get_mayor_name, set_mayor_name, mayor_name;
    get_utility_advisor_name, set_utility_advisor_name, utility_advisor_name;
    get_description, set_description, description;
}
impl<C: ConnectionHandle, D: DealHandle> RegionalCity<C, D> {
    pub fn get_world_position(&self) -> (f32, f32) {
        (self.world[0], self.world[1])
    }
    pub fn set_world_position(&mut self, longitude: f32, latitude: f32) {
        self.world[0] = longitude;
        self.world[1] = latitude;
    }
    pub fn get_world_base_elevation(&self) -> f32 {
        self.world[2]
    }
    pub fn set_world_base_elevation(&mut self, elevation: f32) {
        self.world[2] = elevation;
    }
    pub fn get_world_hemisphere(&self) -> u32 {
        self.world_hemisphere()
    }
    pub fn get_position(&self) -> (i32, i32) {
        self.position
    }
    pub fn get_total_population(&self) -> i32 {
        self.population
    }
    pub fn get_commercial_jobs(&self) -> i32 {
        self.commercial_jobs
    }
    pub fn get_industrial_jobs(&self) -> i32 {
        self.industrial_jobs
    }
    pub fn get_mayor_rating(&self) -> i8 {
        self.mayor_rating
    }
    pub fn get_difficulty_level(&self) -> u8 {
        self.difficulty
    }
}

fn c_string(s: &[u8]) -> Vec<u8> {
    s.iter().copied().take_while(|&b| b != 0).collect()
}
fn trunc_i32(v: f32) -> i32 {
    if v.is_finite() && (-2147483648.0..2147483648.0).contains(&v) {
        v.trunc() as i32
    } else {
        i32::MIN
    }
}

pub trait DealRegion {
    /// None means no current city or no neighbours simulator.
    fn current_city(&self) -> Option<CityId>;
    fn city_at(&self, cell: (i32, i32)) -> Option<CityId>;
}
fn extract<D: DealHandle>(src: &mut Vec<D>, dst: &mut Vec<D>, remove: bool, this: CityId, host: &impl DealRegion) {
    let Some(cur) = host.current_city() else {
        return;
    };
    src.retain(|d| {
        let (Some(a), Some(b)) = (host.city_at(d.city_side()), host.city_at(d.other_side())) else {
            return false;
        };
        let claim = if this == cur {
            a == cur || b == cur
        } else {
            (a == cur && b == this) || (a == this && b == cur)
        };
        if claim {
            dst.push(d.clone());
        }
        !claim || !remove
    });
}

#[derive(Clone, Debug, PartialEq)]
pub struct Preferences {
    pub city_name: Vec<u8>,
    pub mayor_name: Vec<u8>,
    pub difficulty: u8,
}
pub trait CitySetup {
    fn set_mayor_name(&mut self, name: &[u8]);
    fn set_city_name(&mut self, name: &[u8]);
    fn set_difficulty(&mut self, difficulty: u8);
    fn create_save_path(&mut self, name: &[u8]) -> Vec<u8>;
    fn set_file_path(&mut self, path: &[u8]);
    fn set_size(&mut self, metres: (i32, i32));
}
pub trait CacheHost<C = Rc<NeighborConnection>, D = Rc<NeighborDeal>> {
    fn mayor_name(&mut self) -> Vec<u8>;
    fn city_name(&mut self) -> Vec<u8>;
    fn difficulty(&mut self) -> u8;
    fn file_path(&mut self) -> Vec<u8>;
    fn serial(&mut self) -> u32;
    fn established(&mut self) -> bool;
    fn mayor_rating(&mut self) -> i8;
    fn tax_rate(&mut self, index: usize) -> f32;
    fn connections(&mut self) -> Vec<C>;
    fn update_commodity_stats(&mut self, city: &mut RegionalCity<C, D>);
    fn tutorial_id(&mut self) -> Option<i32>;
    fn residential(&mut self) -> Option<(i32, f32)>;
    fn population_summary(&mut self) -> BTreeMap<u32, i32>;
    fn jobs_per_unit(&mut self, population_type: u32) -> [f32; 4];
    fn lots(&mut self, poi_type: u32) -> Vec<(bool, [f32; 3])>;
}
#[derive(Clone, Copy, Debug, Default)]
pub struct MessageContext {
    pub application_object: bool,
    pub city_exists: bool,
    pub names_current_city: bool,
    pub established: bool,
    pub forced: bool,
}
pub trait MessageHost<C = Rc<NeighborConnection>, D = Rc<NeighborDeal>>: CacheHost<C, D> + ConnectionHost<C> {}
impl<C, D, T: CacheHost<C, D> + ConnectionHost<C>> MessageHost<C, D> for T {}

#[cfg(test)]
mod tests;
