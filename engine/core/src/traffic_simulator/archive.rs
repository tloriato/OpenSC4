// SPDX-License-Identifier: GPL-3.0-or-later
//! Standalone traffic archive, save-records §7.1 (authoritative layout).
//! References and container order are saved data, not live host objects/hash
//! tables. Legacy encodings are retained rather than applying simulation load
//! fixups; writing an archive does not upgrade its version or rebuild switches.

use crate::property::PropertyHolder;
use crate::store::archive::ObjectReference;
use crate::stream::{ByteReader, InStream, OutStream};

trait Wire: Sized {
    fn read(r: &mut ByteReader<'_>) -> Option<Self>;
    fn write(&self, w: &mut dyn OutStream);
}

macro_rules! scalar {
    ($ty:ty, $get:ident, $put:ident) => {
        impl Wire for $ty {
            fn read(r: &mut ByteReader<'_>) -> Option<Self> {
                r.$get()
            }
            fn write(&self, w: &mut dyn OutStream) {
                w.$put(*self);
            }
        }
    };
}
scalar!(u8, get_u8, put_u8);
scalar!(u16, get_u16, put_u16);
scalar!(u32, get_u32, put_u32);
scalar!(i16, get_i16, put_i16);
scalar!(i32, get_i32, put_i32);
scalar!(i64, get_i64, put_i64);
scalar!(f32, get_f32, put_f32);
scalar!(f64, get_f64, put_f64);

impl Wire for ObjectReference {
    fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        Self::read(r)
    }
    fn write(&self, w: &mut dyn OutStream) {
        (*self).write(w);
    }
}

impl<T: Wire + Copy + Default, const N: usize> Wire for [T; N] {
    fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        let mut values = [T::default(); N];
        for value in &mut values {
            *value = T::read(r)?;
        }
        Some(values)
    }
    fn write(&self, w: &mut dyn OutStream) {
        for value in self {
            value.write(w);
        }
    }
}

macro_rules! fields {
    ($name:ident { $($field:ident: $ty:ty),* $(,)? }) => {
        #[derive(Clone, Copy, Debug, Default)]
        pub struct $name { $(pub $field: $ty),* }
        impl Wire for $name {
            fn read(r: &mut ByteReader<'_>) -> Option<Self> {
                Some(Self { $($field: <$ty>::read(r)?),* })
            }
            fn write(&self, w: &mut dyn OutStream) { $(self.$field.write(w);)* }
        }
    };
}

fields!(Rule {
    kind: u8,
    sides: [u8; 4],
    from: u16,
    to: u16
});
fields!(SwitchValues {
    building: ObjectReference,
    load: f32,
    time_factor: f32,
    capacity: f32,
});
fields!(SwitchAccess {
    has_access: u8,
    cell: [u8; 2],
    counters: [f32; 2],
});
fields!(Scan {
    state: u8,
    cell: [i32; 2],
    step: u32,
    rectangle: [i32; 4],
});
fields!(Context {
    flags: [u8; 2],
    value: f32,
    cell: [i32; 2],
    word: u32,
    words: [u32; 4],
    byte: u8,
    word2: u32,
    integers: [i32; 4],
});
fields!(StatRow {
    sum: f32,
    count: u32,
    max_cell: [i32; 2],
    max: f32
});
fields!(SwitchStat {
    count: u16,
    value: f32,
    integers: [i32; 2],
    value2: f32
});
fields!(StatTail {
    values: [f32; 2],
    flag: u8,
    value: f32
});
fields!(TunnelHeader {
    key: u16,
    other_cell: [u8; 2],
    mask: u16,
    network: u16
});
fields!(FerryHeader {
    cells: [u8; 6],
    ferry_type: u8,
    mask: u16,
    riders: [u16; 2],
});
fields!(TerminalLinkHeader {
    cost: f32,
    arrival: [i16; 2],
    departure: u16
});
fields!(NeighborLinkHeader {
    cost: f32,
    cells: [i16; 4]
});

#[derive(Clone, Debug)]
pub enum Density {
    Current {
        last: [u16; 16],
        usage: u16,
        current: [u16; 16],
    },
    Legacy([f32; 34]),
}

#[derive(Clone, Debug, Default)]
pub struct Switch {
    pub id: u32,
    pub values: SwitchValues,
    pub legacy_route: Vec<u8>,
    pub access: SwitchAccess,
    pub rules: Vec<Rule>,
    pub toll: f32,
    pub intersection: u8,
}

#[derive(Clone, Debug, Default)]
pub struct Damage {
    pub cells: u32,
    pub damaged: u32,
    pub rate: f32,
    pub intact_keys: Vec<u16>,
    pub damaged_keys: Vec<u16>,
}

#[derive(Clone, Debug, Default)]
pub struct Statistics {
    pub rows: Vec<StatRow>,
    pub bridges: Vec<StatRow>,
    pub records: Vec<u16>,
    pub bridge_records: Vec<u16>,
    pub potholes: u16,
    pub switches: Vec<SwitchStat>,
    pub tail: StatTail,
    pub byte: u8,
}

#[derive(Clone, Debug, Default)]
pub struct Ferry {
    pub legacy_word: u16,
    pub header: FerryHeader,
    pub lot_cell: [u8; 2],
    pub terminals: Vec<TerminalLink>,
    pub neighbors: Vec<NeighborLink>,
}

#[derive(Clone, Debug, Default)]
pub struct TerminalLink {
    pub header: TerminalLinkHeader,
    pub route: Vec<u8>,
    /// Disk order (in-memory counters 0, 2, 1, 3).
    pub counters: [u16; 4],
}

#[derive(Clone, Debug, Default)]
pub struct NeighborLink {
    pub header: NeighborLinkHeader,
    pub route_out: Vec<u8>,
    pub route_back: Vec<u8>,
    pub counters: [u16; 4],
}

#[derive(Clone, Debug, Default)]
pub struct TrafficRecord {
    pub version: u16,
    pub dimensions: [i32; 4],
    pub published: [ObjectReference; 6],
    pub destination_size: [u32; 2],
    pub destinations: Vec<u8>,
    pub volumes: Vec<ObjectReference>,
    pub float_grids: [ObjectReference; 2],
    pub densities: Vec<(u32, Density)>,
    pub jobs: [[u32; 3]; 3],
    pub switch_grids: [ObjectReference; 2],
    pub next_switch_id: u32,
    pub switches: Vec<Switch>,
    pub departments: [ObjectReference; 2],
    pub budget_total: i64,
    pub transit_income: f32,
    pub toll_income: f32,
    pub last_income: f32,
    pub income_accumulator: f64,
    /// v19–21 stores a float, retained without a widening/narrowing operation.
    pub legacy_income_accumulator: f32,
    pub fee_accumulator: f64,
    pub cycle_start: u32,
    pub strike: u8,
    pub strike_months: i32,
    pub road: Damage,
    pub rail: Damage,
    pub cycle_state: u8,
    pub scan: Scan,
    pub intersection_dirty: u8,
    pub intersection_rectangle: [i32; 4],
    pub network_changed: u8,
    pub strategy: [u32; 2],
    pub phase_positions: [i16; 2],
    pub lot_iterator: [u8; 30],
    pub context: Context,
    pub context_table: [Vec<u32>; 3],
    pub context_flags: [u8; 2],
    pub processed: i32,
    pub day_index: i32,
    pub total: i32,
    pub days: u32,
    pub pacing_unused: [u32; 3],
    pub pass_totals: [i32; 3],
    pub pass_counters: [u32; 10],
    pub statistics: [Statistics; 2],
    pub tunnels: Vec<(TunnelHeader, Vec<f32>)>,
    pub avg_car_commute: f32,
    pub car_length: f32,
    pub car_count: u32,
    pub freight_length: f32,
    pub freight_count: u32,
    pub cells_per_network: Vec<u32>,
    pub free_destinations: Vec<u32>,
    pub pools: Vec<(u16, PropertyHolder)>,
    pub travel_counts: Vec<[u32; 2]>,
    pub ferry_total: u32,
    pub ferries: Vec<Ferry>,
}

// Bounds come from the remaining payload, not city dimensions or guessed limits.
fn list<T>(
    r: &mut ByteReader<'_>,
    min_size: usize,
    mut read: impl FnMut(&mut ByteReader<'_>) -> Option<T>,
) -> Option<Vec<T>> {
    let count = r.get_u32()? as usize;
    fixed(r, count, min_size, &mut read)
}

fn fixed<T>(
    r: &mut ByteReader<'_>,
    count: usize,
    min_size: usize,
    mut read: impl FnMut(&mut ByteReader<'_>) -> Option<T>,
) -> Option<Vec<T>> {
    if count > r.remaining() / min_size {
        return None;
    }
    (0..count).map(|_| read(r)).collect()
}

fn put_list<T>(w: &mut dyn OutStream, values: &[T], mut write: impl FnMut(&T, &mut dyn OutStream)) {
    w.put_u32(values.len() as u32);
    for value in values {
        write(value, w);
    }
}

fn put_fixed<T: Wire>(w: &mut dyn OutStream, values: &[T]) {
    for value in values {
        value.write(w);
    }
}

impl Damage {
    fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        Some(Self {
            cells: r.get_u32()?,
            damaged: r.get_u32()?,
            rate: r.get_f32()?,
            intact_keys: list(r, 2, u16::read)?,
            damaged_keys: list(r, 2, u16::read)?,
        })
    }
    fn write(&self, w: &mut dyn OutStream) {
        self.cells.write(w);
        self.damaged.write(w);
        self.rate.write(w);
        put_list(w, &self.intact_keys, Wire::write);
        put_list(w, &self.damaged_keys, Wire::write);
    }
}

impl Statistics {
    fn read(r: &mut ByteReader<'_>, v: u16, [a, b, c, _]: [usize; 4]) -> Option<Self> {
        let rows = fixed(r, if v >= 35 { a } else { c }, 20, StatRow::read)?;
        let bridges = fixed(r, if v >= 37 { b } else { 0 }, 20, StatRow::read)?;
        let records = fixed(r, a, 2, u16::read)?;
        let bridge_records = fixed(r, if v >= 37 { b } else { 0 }, 2, u16::read)?;
        let potholes = r.get_u16()?;
        let count = if v >= 34 {
            usize::try_from(r.get_i32()?).ok()?
        } else {
            3
        };
        Some(Self {
            rows,
            bridges,
            records,
            bridge_records,
            potholes,
            switches: fixed(r, count, 18, SwitchStat::read)?,
            tail: StatTail::read(r)?,
            byte: if v >= 21 { r.get_u8()? } else { 0x80 },
        })
    }
    fn write(&self, w: &mut dyn OutStream, v: u16) {
        put_fixed(w, &self.rows);
        put_fixed(w, &self.bridges);
        put_fixed(w, &self.records);
        put_fixed(w, &self.bridge_records);
        self.potholes.write(w);
        if v >= 34 {
            (self.switches.len() as i32).write(w);
        }
        put_fixed(w, &self.switches);
        self.tail.write(w);
        if v >= 21 {
            self.byte.write(w);
        }
    }
}

impl Switch {
    fn read(r: &mut ByteReader<'_>, v: u16) -> Option<Self> {
        Some(Self {
            id: r.get_u32()?,
            values: SwitchValues::read(r)?,
            legacy_route: if v < 29 { list(r, 1, u8::read)? } else { Vec::new() },
            access: SwitchAccess::read(r)?,
            rules: if v >= 29 { list(r, 9, Rule::read)? } else { Vec::new() },
            toll: if v >= 33 { r.get_f32()? } else { 0.0 },
            intersection: if v >= 33 { r.get_u8()? } else { 0 },
        })
    }
    fn write(&self, w: &mut dyn OutStream, v: u16) {
        self.id.write(w);
        self.values.write(w);
        if v < 29 {
            put_list(w, &self.legacy_route, Wire::write);
        }
        self.access.write(w);
        if v >= 29 {
            put_list(w, &self.rules, Wire::write);
        }
        if v >= 33 {
            self.toll.write(w);
            self.intersection.write(w);
        }
    }
}

fn route(r: &mut ByteReader<'_>) -> Option<Vec<u8>> {
    let count = r.get_u32()? as usize;
    // save-records §7.1.1: counts >= one million consume no route bytes.
    // Such a discarded count has no archive value; reject rather than silently
    // claiming an exact encoding for it.
    if count >= 1_000_000 {
        return None;
    }
    r.get_bytes(count)
}

impl Ferry {
    fn read(r: &mut ByteReader<'_>, v: u16) -> Option<Self> {
        Some(Self {
            legacy_word: if v < 32 { r.get_u16()? } else { 0 },
            header: FerryHeader::read(r)?,
            lot_cell: if v >= 38 { <[u8; 2]>::read(r)? } else { [0; 2] },
            terminals: list(r, 14, |r| {
                Some(TerminalLink {
                    header: TerminalLinkHeader::read(r)?,
                    route: route(r)?,
                    counters: if v >= 36 { <[u16; 4]>::read(r)? } else { [0; 4] },
                })
            })?,
            neighbors: list(r, 16, |r| {
                Some(NeighborLink {
                    header: NeighborLinkHeader::read(r)?,
                    route_out: route(r)?,
                    route_back: if v >= 31 { route(r)? } else { Vec::new() },
                    counters: if v >= 36 { <[u16; 4]>::read(r)? } else { [0; 4] },
                })
            })?,
        })
    }
    fn write(&self, w: &mut dyn OutStream, v: u16) {
        if v < 32 {
            self.legacy_word.write(w);
        }
        self.header.write(w);
        if v >= 38 {
            self.lot_cell.write(w);
        }
        put_list(w, &self.terminals, |link, w| {
            link.header.write(w);
            put_list(w, &link.route, Wire::write);
            if v >= 36 {
                link.counters.write(w);
            }
        });
        put_list(w, &self.neighbors, |link, w| {
            link.header.write(w);
            put_list(w, &link.route_out, Wire::write);
            if v >= 31 {
                put_list(w, &link.route_back, Wire::write);
            }
            if v >= 36 {
                link.counters.write(w);
            }
        });
    }
}

impl TrafficRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        let v = r.get_u16()?;
        if v < 19 {
            return None;
        }
        let dimensions = [
            if v >= 24 { r.get_i32()? } else { 8 },
            if v >= 37 { r.get_i32()? } else { 1 },
            if v >= 28 { r.get_i32()? } else { 4 },
            if v >= 28 { r.get_i32()? } else { 7 },
        ];
        // Dimensions are used for fixed tables later; reject impossible sizes
        // before multiplying them or allocating those tables.
        if dimensions.iter().any(|&n| n < 0 || n as usize > r.remaining()) {
            return None;
        }
        let [a, _, _, d] = dimensions.map(|n| n as usize);
        let published = <[ObjectReference; 6]>::read(r)?;
        let destination_size = <[u32; 2]>::read(r)?;
        let cells = (destination_size[0] as usize).checked_mul(destination_size[1] as usize)?;
        let destinations = r.get_bytes(cells)?;
        let volumes = list(r, 4, <ObjectReference as Wire>::read)?;
        let float_grids = <[ObjectReference; 2]>::read(r)?;
        let densities = list(r, 70, |r| {
            let key = r.get_u32()?;
            let density = if v >= 20 {
                Density::Current {
                    last: <[u16; 16]>::read(r)?,
                    usage: r.get_u16()?,
                    current: <[u16; 16]>::read(r)?,
                }
            } else {
                Density::Legacy(<[f32; 34]>::read(r)?)
            };
            Some((key, density))
        })?;
        let jobs = <[[u32; 3]; 3]>::read(r)?;
        let switch_grids = <[ObjectReference; 2]>::read(r)?;
        let next_switch_id = r.get_u32()?;
        let switches = list(r, 31, |r| Switch::read(r, v))?;
        let departments = <[ObjectReference; 2]>::read(r)?;
        let budget_total = r.get_i64()?;
        let transit_income = r.get_f32()?;
        let toll_income = if v >= 41 { r.get_f32()? } else { 0.0 };
        let last_income = r.get_f32()?;
        let income_accumulator = if v >= 22 { r.get_f64()? } else { 0.0 };
        let legacy_income_accumulator = if v < 22 { r.get_f32()? } else { 0.0 };
        let fee_accumulator = if v >= 41 { r.get_f64()? } else { 0.0 };
        let cycle_start = r.get_u32()?;
        let strike = r.get_u8()?;
        let strike_months = r.get_i32()?;
        let road = Damage::read(r)?;
        let rail = if v >= 26 { Damage::read(r)? } else { Damage::default() };
        let cycle_state = r.get_u8()?;
        let scan = Scan::read(r)?;
        let intersection_dirty = r.get_u8()?;
        let intersection_rectangle = <[i32; 4]>::read(r)?;
        let network_changed = r.get_u8()?;
        let strategy = <[u32; 2]>::read(r)?;
        let phase_positions = if v >= 25 {
            <[i16; 2]>::read(r)?
        } else {
            <[u8; 2]>::read(r)?.map(i16::from)
        };
        let lot_iterator = <[u8; 30]>::read(r)?;
        let context = Context::read(r)?;
        let context_table = [
            fixed(r, d, 4, u32::read)?,
            fixed(r, d, 4, u32::read)?,
            fixed(r, d, 4, u32::read)?,
        ];
        let context_flags = <[u8; 2]>::read(r)?;
        let processed = r.get_i32()?;
        let day_index = r.get_i32()?;
        let total = if v >= 25 { r.get_i32()? } else { 0 };
        let days = r.get_u32()?;
        let pacing_unused = <[u32; 3]>::read(r)?;
        let pass_totals = if v >= 25 { <[i32; 3]>::read(r)? } else { [0; 3] };
        let mut pass_counters = [0; 10];
        for value in &mut pass_counters[..if v >= 40 {
            10
        } else if v >= 25 {
            4
        } else {
            0
        }] {
            *value = r.get_u32()?;
        }
        let dims = dimensions.map(|n| n as usize);
        let statistics = [Statistics::read(r, v, dims)?, Statistics::read(r, v, dims)?];
        let tunnels = list(r, 8, |r| Some((TunnelHeader::read(r)?, fixed(r, d, 4, f32::read)?)))?;
        let avg_car_commute = r.get_f32()?;
        let car_length = r.get_f32()?;
        let car_count = r.get_u32()?;
        let freight_length = if v >= 21 { r.get_f32()? } else { 0.0 };
        let freight_count = if v >= 21 { r.get_u32()? } else { 0 };
        let cells_per_network = fixed(r, a, 4, u32::read)?;
        let free_destinations = list(r, 4, u32::read)?;
        let pools = list(r, 8, |r| {
            let key = r.get_u16()?;
            let mut holder = PropertyHolder::new();
            holder.read(r).then_some((key, holder))
        })?;
        let travel_counts = if v >= 27 {
            fixed(r, d, 8, <[u32; 2]>::read)?
        } else {
            Vec::new()
        };
        let ferry_total = if v >= 39 { r.get_u32()? } else { 0 };
        let ferries = if v >= 30 {
            list(r, 21, |r| Ferry::read(r, v))?
        } else {
            Vec::new()
        };
        Some(Self {
            version: v,
            dimensions,
            published,
            destination_size,
            destinations,
            volumes,
            float_grids,
            densities,
            jobs,
            switch_grids,
            next_switch_id,
            switches,
            departments,
            budget_total,
            transit_income,
            toll_income,
            last_income,
            income_accumulator,
            legacy_income_accumulator,
            fee_accumulator,
            cycle_start,
            strike,
            strike_months,
            road,
            rail,
            cycle_state,
            scan,
            intersection_dirty,
            intersection_rectangle,
            network_changed,
            strategy,
            phase_positions,
            lot_iterator,
            context,
            context_table,
            context_flags,
            processed,
            day_index,
            total,
            days,
            pacing_unused,
            pass_totals,
            pass_counters,
            statistics,
            tunnels,
            avg_car_commute,
            car_length,
            car_count,
            freight_length,
            freight_count,
            cells_per_network,
            free_destinations,
            pools,
            travel_counts,
            ferry_total,
            ferries,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        let v = self.version;
        v.write(w);
        for (i, gate) in [24, 37, 28, 28].into_iter().enumerate() {
            if v >= gate {
                self.dimensions[i].write(w);
            }
        }
        self.published.write(w);
        self.destination_size.write(w);
        w.put_bytes(&self.destinations);
        put_list(w, &self.volumes, Wire::write);
        self.float_grids.write(w);
        put_list(w, &self.densities, |(key, density), w| {
            key.write(w);
            match density {
                Density::Current { last, usage, current } => {
                    last.write(w);
                    usage.write(w);
                    current.write(w);
                }
                Density::Legacy(values) => values.write(w),
            }
        });
        self.jobs.write(w);
        self.switch_grids.write(w);
        self.next_switch_id.write(w);
        put_list(w, &self.switches, |s, w| s.write(w, v));
        self.departments.write(w);
        self.budget_total.write(w);
        self.transit_income.write(w);
        if v >= 41 {
            self.toll_income.write(w);
        }
        self.last_income.write(w);
        if v >= 22 {
            self.income_accumulator.write(w);
        } else {
            self.legacy_income_accumulator.write(w);
        }
        if v >= 41 {
            self.fee_accumulator.write(w);
        }
        self.cycle_start.write(w);
        self.strike.write(w);
        self.strike_months.write(w);
        self.road.write(w);
        if v >= 26 {
            self.rail.write(w);
        }
        self.cycle_state.write(w);
        self.scan.write(w);
        self.intersection_dirty.write(w);
        self.intersection_rectangle.write(w);
        self.network_changed.write(w);
        self.strategy.write(w);
        if v >= 25 {
            self.phase_positions.write(w);
        } else {
            self.phase_positions.map(|p| p as u8).write(w);
        }
        self.lot_iterator.write(w);
        self.context.write(w);
        for row in &self.context_table {
            put_fixed(w, row);
        }
        self.context_flags.write(w);
        self.processed.write(w);
        self.day_index.write(w);
        if v >= 25 {
            self.total.write(w);
        }
        self.days.write(w);
        self.pacing_unused.write(w);
        if v >= 25 {
            self.pass_totals.write(w);
        }
        put_fixed(
            w,
            &self.pass_counters[..if v >= 40 {
                10
            } else if v >= 25 {
                4
            } else {
                0
            }],
        );
        for stats in &self.statistics {
            stats.write(w, v);
        }
        put_list(w, &self.tunnels, |(header, costs), w| {
            header.write(w);
            put_fixed(w, costs);
        });
        self.avg_car_commute.write(w);
        self.car_length.write(w);
        self.car_count.write(w);
        if v >= 21 {
            self.freight_length.write(w);
            self.freight_count.write(w);
        }
        put_fixed(w, &self.cells_per_network);
        put_list(w, &self.free_destinations, Wire::write);
        put_list(w, &self.pools, |(key, holder), w| {
            key.write(w);
            holder.write(w);
        });
        if v >= 27 {
            put_fixed(w, &self.travel_counts);
        }
        if v >= 39 {
            self.ferry_total.write(w);
        }
        if v >= 30 {
            put_list(w, &self.ferries, |f, w| f.write(w, v));
        }
        w.error() == 0
    }
}

#[cfg(test)]
mod tests;
