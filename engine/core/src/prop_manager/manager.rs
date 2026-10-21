// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The prop manager component (§A1, §A2, §A4, §A6–§A8).

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::property::PropertyHolder;
use crate::props;
use crate::stream::{InStream, OutStream, RefValue, ResKey, SaveObject};

use super::catalogue::{PropCatalogue, PropProfile, SharedPropCatalogue};
use super::{
    class, msg, prop, usage, BuildingProps, BuiltinProp, CellRect, CommandArg, PropHost, PropMessage, PropService,
    WorldRect, CLSID_PROP_MANAGER, COMMAND_CREATE_TEXT_DECAL, FLAG_INSERTED, FLAG_SHOWN, IID_LOT_MANAGER,
    IID_MESSAGE_TARGET2, IID_OCCUPANT_MANAGER, IID_PROP_MANAGER, IID_SERIALIZABLE, IID_UNKNOWN, RECORD_VERSION,
};

/// `QueryInterface` (§A1): `0x28326F3E`, `1`, `cIGZMessageTarget2` and
/// `cIGZSerializable`.
pub fn query_interface(iid: u32) -> bool {
    matches!(
        iid,
        IID_PROP_MANAGER | IID_UNKNOWN | IID_MESSAGE_TARGET2 | IID_SERIALIZABLE
    )
}

/// Every field of §A2 (the catalogue is shared, see [`PropCatalogue`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PropState {
    /// +0x0C.
    pub initialized: bool,
    /// +0x10.
    pub ref_count: u32,
    /// The last `Release` destroyed the object.
    pub destroyed: bool,
    /// +0x14 (saved).
    pub occupant_manager: Option<u64>,
    /// +0x18.
    pub resource_manager: Option<u64>,
    /// +0x1C: the occupant filter exists.
    pub filter: bool,
    /// The filter's usage (+0xC of the filter object).
    pub filter_usage: u32,
    /// +0x20 (saved).
    pub lot_manager: Option<u64>,
    /// +0x24.
    pub view_filter: Option<u64>,
}

/// The prop manager.
pub struct PropManager {
    host: Rc<dyn PropHost>,
    catalogue: SharedPropCatalogue,
    token: Cell<u64>,
    s: RefCell<PropState>,
}

impl PropManager {
    /// A constructed manager (every field zero, §A1), sharing the process
    /// catalogue `catalogue`. `token` is the manager's own host identity
    /// (void1 of the catalogue message).
    pub fn new(host: Rc<dyn PropHost>, catalogue: SharedPropCatalogue, token: u64) -> Rc<PropManager> {
        Rc::new(PropManager {
            host,
            catalogue,
            token: Cell::new(token),
            s: RefCell::new(PropState::default()),
        })
    }

    pub fn token(&self) -> u64 {
        self.token.get()
    }

    pub fn set_token(&self, token: u64) {
        self.token.set(token);
    }

    pub fn catalogue(&self) -> SharedPropCatalogue {
        self.catalogue.clone()
    }

    pub fn snapshot(&self) -> PropState {
        self.s.borrow().clone()
    }

    pub fn with_state<R>(&self, f: impl FnOnce(&mut PropState) -> R) -> R {
        f(&mut self.s.borrow_mut())
    }

    // ---- identity (§A1) ----

    pub fn add_ref(&self) -> u32 {
        let mut s = self.s.borrow_mut();
        s.ref_count = s.ref_count.wrapping_add(1);
        s.ref_count
    }

    /// `Release` (`0x004A2340`): above 1 it decrements; the last one calls
    /// `Shutdown` and destroys the object (here: marks it), returning 0.
    pub fn release(&self) -> u32 {
        let c = self.s.borrow().ref_count;
        if c > 1 {
            self.s.borrow_mut().ref_count = c - 1;
            return c - 1;
        }
        self.shutdown();
        let mut s = self.s.borrow_mut();
        s.ref_count = 0;
        s.destroyed = true;
        0
    }

    // ---- the catalogue (§A3) ----

    /// `StaticInit` (`0x004A4310`, §A3.1): builds the catalogue from every
    /// exemplar of type `0x1E`, registers the shutdown callback and posts
    /// the catalogue message. Called by the first `Init` of the process.
    pub fn static_init(&self) {
        let h = &*self.host;
        {
            let mut c = self.catalogue.borrow_mut();
            c.set_built();
            c.set_callback_registered();
        }
        h.register_shutdown_callback();
        if let Some(keys) = h.exemplar_keys() {
            for key in keys {
                let Some(ex) = h.load_exemplar(key) else {
                    continue;
                };
                if PropCatalogue::is_prop_exemplar(&ex) {
                    self.catalogue.borrow_mut().add_exemplar(key, &ex);
                }
            }
        }
        let data2 = self.catalogue.borrow().data2();
        h.post_exemplar_data_updated(self.token.get(), data2);
    }

    // ---- lifecycle (§A4) ----

    /// Slot 3 `Init` (§A4.1). True.
    pub fn init(&self) -> bool {
        let h = &*self.host;
        {
            let mut s = self.s.borrow_mut();
            if s.initialized {
                return true;
            }
            s.initialized = true;
        }
        if !self.catalogue.borrow().is_built() {
            self.static_init();
        }
        h.register_command(COMMAND_CREATE_TEXT_DECAL);
        for m in msg::SUBSCRIPTIONS {
            h.subscribe(m);
        }
        if self.s.borrow().occupant_manager.is_none() {
            let t = h.acquire_service(PropService::OccupantManager);
            self.s.borrow_mut().occupant_manager = t;
        }
        if self.s.borrow().resource_manager.is_none() {
            let t = h.acquire_service(PropService::ResourceManager);
            self.s.borrow_mut().resource_manager = t;
        }
        {
            let mut s = self.s.borrow_mut();
            if !s.filter {
                s.filter = true;
                s.filter_usage = 0;
            }
        }
        if self.s.borrow().lot_manager.is_none() {
            let t = h.acquire_service(PropService::LotManager);
            self.s.borrow_mut().lot_manager = t;
        }
        // [quirk] The 3D view itself is not tested: the host answers None.
        if self.s.borrow().view_filter.is_none() {
            let t = h.view_filter();
            self.s.borrow_mut().view_filter = t;
        }
        true
    }

    /// Slot 4 `Shutdown` (§A4.2). The catalogue stays. True.
    pub fn shutdown(&self) -> bool {
        let h = &*self.host;
        let (vf, lm, om, rm) = {
            let mut s = self.s.borrow_mut();
            if !s.initialized {
                return true;
            }
            s.initialized = false;
            s.filter = false;
            (
                s.view_filter.take(),
                s.lot_manager.take(),
                s.occupant_manager.take(),
                s.resource_manager.take(),
            )
        };
        // Order: view filter, lot manager, (filter), resource manager,
        // occupant manager.
        for t in [vf, lm, rm, om].into_iter().flatten() {
            h.release(t);
        }
        for m in msg::SUBSCRIPTIONS {
            h.unsubscribe(m);
        }
        h.unregister_command(COMMAND_CREATE_TEXT_DECAL);
        true
    }

    // ---- messages (§A4.3) ----

    /// `DoMessage`. True for every message.
    pub fn do_message(&self, m: &PropMessage) -> bool {
        match m.msg_type {
            msg::POST_VIEW_FILTER_CHANGE => {
                let old = self.s.borrow().view_filter;
                if m.view_filter != old {
                    if let Some(n) = m.view_filter {
                        self.host.add_ref(n);
                    }
                    if let Some(o) = old {
                        self.host.release(o);
                    }
                    self.s.borrow_mut().view_filter = m.view_filter;
                }
            }
            msg::CREATE_TEXT_DECAL => self.create_text_decal(&m.args),
            // 0xA6B79602 reads the data and does nothing; PostCityInit is
            // ignored.
            _ => {}
        }
        true
    }

    /// The `CreateTextDecal` debug command (§A4.3). Needs more than two
    /// arguments; the handler's own reference is never released (quirk 9).
    fn create_text_decal(&self, args: &[CommandArg]) {
        let h = &*self.host;
        let n = args.len();
        if n <= 2 {
            return;
        }
        let text = args[0].text.clone();
        let x = args[1].value;
        let z = args[2].value;
        let a3 = if n > 3 { args[3].value } else { 1.0 };
        let a4_7 = if n > 7 {
            [args[4].value, args[5].value, args[6].value, args[7].value]
        } else {
            [1.0; 4]
        };
        let Some(decal) = h.create_text_decal() else {
            return;
        };
        self.add_city_prop(decal);
        let y = h.terrain_altitude(x, z);
        h.set_position(decal, [x, y, z]);
        h.decal_set_text(decal, &text);
        h.decal_slot8(decal, [a4_7[0], a4_7[1], a4_7[2]]);
        h.decal_slot10(decal, a4_7[3]);
        h.decal_slot6(decal, a3);
    }

    // ---- catalogue queries (§A5, slots 5–14) ----

    pub fn all_prop_types(&self) -> Vec<u32> {
        self.catalogue.borrow().all_prop_types()
    }
    pub fn all_prop_family_types(&self) -> Vec<u32> {
        self.catalogue.borrow().all_family_types()
    }
    pub fn prop_key_from_type(&self, id: u32) -> Option<ResKey> {
        self.catalogue.borrow().key_of(id)
    }
    pub fn is_prop_type_valid(&self, id: u32) -> bool {
        self.catalogue.borrow().is_prop_type_valid(id)
    }
    pub fn prop_types_by_family(&self, vec: &mut Vec<u32>, id: u32) -> u32 {
        self.catalogue.borrow().prop_types_by_family(vec, id)
    }
    pub fn is_prop_in_family(&self, ty: u32, family: u32) -> bool {
        self.catalogue.borrow().is_prop_in_family(ty, family)
    }
    pub fn is_prop_family_type_valid(&self, id: u32) -> bool {
        self.catalogue.borrow().is_family_type_valid(id)
    }
    pub fn prop_name(&self, id: u32) -> String {
        self.catalogue.borrow_mut().prop_name(id)
    }
    pub fn prop_profile(&self, id: u32) -> PropProfile {
        self.catalogue.borrow_mut().prop_profile(id)
    }
    pub fn prop_profile_ptr(&self, id: u32) -> Option<PropProfile> {
        self.catalogue.borrow().prop_profile_ptr(id)
    }

    // ---- creating props (§A6) ----

    /// The class of §A6.1 step 2–3: `class_id` (0 = the default class),
    /// asked from GZCOM, else a built-in class. One reference. GLUE21-2
    /// (prop-developer.md §13.2): the original, when GZCOM does not answer,
    /// allocates and constructs the occupant itself; the host's
    /// `create_builtin_prop` is that fallback and 0 means neither route
    /// produced an occupant.
    fn instantiate(&self, class_id: u32) -> u64 {
        let c = if class_id == 0 { class::DEFAULT_PROP } else { class_id };
        self.host
            .create_prop_instance(c)
            .unwrap_or_else(|| self.host.create_builtin_prop(BuiltinProp::for_class(c)))
    }

    /// §A6.1 step 5's bounding box from the loaded exemplar: (0,0,0) to
    /// OccupantSize, or (1, 1, 1) without one; missing values 0 [choice].
    fn size_prop(&self, p: u64, ex: &PropertyHolder) {
        let max = match props::f32s(ex, prop::OCCUPANT_SIZE) {
            Some(v) if !v.is_empty() => [v[0], v.get(1).copied().unwrap_or(0.0), v.get(2).copied().unwrap_or(0.0)],
            _ => [1.0, 1.0, 1.0],
        };
        self.host.set_bounding_box(p, [0.0; 3], max);
    }

    /// Slot 15 `CreateProp(id)` (§A6.1): the new prop (one reference, the
    /// caller's), or `None` when `id` is not a prop type or no occupant
    /// could be made. GLUE21-2 (prop-developer.md §13.2): the original
    /// never hands back a null occupant - it validates the type, tries
    /// GZCOM and otherwise allocates and constructs the occupant itself.
    /// FreeCity4 has no route to that allocate-and-construct pair yet, so a
    /// failed instantiation fails the call (the original's other exit,
    /// §13.2 step 1) and every later step is skipped: setting the type on
    /// or sizing a null token is what handed the game a null prop occupant
    /// and faulted at `SimCity 4.exe+0x2F44EF`.
    pub fn create_prop(&self, id: u32) -> Option<u64> {
        if !self.is_prop_type_valid(id) {
            return None;
        }
        let class_id = self.prop_profile(id).class_id();
        let p = self.instantiate(class_id);
        if p == 0 {
            return None;
        }
        self.host.set_prop_type(p, id);
        // Step 5: any missing piece skips the rest; always succeeds.
        if let Some(key) = self.prop_key_from_type(id) {
            if let Some(ex) = self.host.attach_exemplar(p, key, false) {
                self.size_prop(p, &ex);
            }
        }
        Some(p)
    }

    /// Slot 16 `CreateGenericProp(key)` (§A6.2): `None` when the exemplar
    /// does not load through the global resource manager or no occupant
    /// could be made (GLUE21-2, as `create_prop`). The prop type is
    /// not set.
    pub fn create_generic_prop(&self, key: ResKey) -> Option<u64> {
        let ex = self.host.load_exemplar(key)?;
        let class_id = props::u32_of(&ex, prop::OCCUPANT_CLASS).unwrap_or(0);
        let p = self.instantiate(class_id);
        if p == 0 {
            return None;
        }
        if let Some(ex) = self.host.attach_exemplar(p, key, true) {
            self.size_prop(p, &ex);
        }
        Some(p)
    }

    // ---- props in the city (§A7) ----

    fn has_occupant_manager(&self) -> bool {
        self.s.borrow().occupant_manager.is_some()
    }

    fn set_filter_usage(&self, u: u32) {
        self.s.borrow_mut().filter_usage = u;
    }

    /// `Init` then `InsertOccupant`, short-circuiting *[inferred]*.
    fn init_and_insert(&self, occ: u64) -> bool {
        self.host.occupant_init(occ) && self.host.insert_occupant(occ)
    }

    /// Slots 19 / 25 (§A7.1): an existing occupant.
    fn add_existing(&self, occ: u64, u: u32) -> bool {
        if !self.has_occupant_manager() {
            return false;
        }
        if self.host.occupant_flag(occ, FLAG_INSERTED) {
            return true;
        }
        if !self.init_and_insert(occ) {
            return false;
        }
        self.host.set_prop_usage(occ, u);
        true
    }

    /// Slot 19 `AddCityProp(occ)`.
    pub fn add_city_prop(&self, occ: u64) -> bool {
        self.add_existing(occ, usage::CITY)
    }

    /// Slot 25 `AddLotProp(lot, occ)`: the lot is ignored (quirk 6).
    pub fn add_lot_prop(&self, _lot: Option<u64>, occ: u64) -> bool {
        self.add_existing(occ, usage::LOT)
    }

    /// `0x004A2010`: slots 20, 26, 32. The occupant manager is not tested
    /// (quirk 5). Returns the occupant (no reference for the caller) or
    /// `None`.
    fn add_new(&self, id: u32, pos: [f32; 3], orientation: i32, u: u32) -> Option<u64> {
        let p = self.create_prop(id)?;
        self.host.set_position(p, pos);
        self.host.set_orientation(p, orientation);
        let ok = self.init_and_insert(p);
        if ok {
            self.host.set_prop_usage(p, u);
        }
        self.host.release(p);
        ok.then_some(p)
    }

    /// Slot 20 `AddCityProp(id, pos, orient)`.
    pub fn add_city_prop_new(&self, id: u32, pos: [f32; 3], orientation: i32) -> Option<u64> {
        self.add_new(id, pos, orientation, usage::CITY)
    }

    /// Slot 26 `AddLotProp(id, pos, orient)`.
    pub fn add_lot_prop_new(&self, id: u32, pos: [f32; 3], orientation: i32) -> Option<u64> {
        self.add_new(id, pos, orientation, usage::LOT)
    }

    /// Slot 32 `AddStreetProp(id, pos, orient)`.
    pub fn add_street_prop(&self, id: u32, pos: [f32; 3], orientation: i32) -> Option<u64> {
        self.add_new(id, pos, orientation, usage::STREET)
    }

    /// Slot 36 `AddBuildingProp(building, prop)` (§A7.1).
    pub fn add_building_prop(&self, building: u64, p: u64) -> bool {
        let h = &*self.host;
        if !self.has_occupant_manager() {
            return false;
        }
        if !h.occupant_flag(p, FLAG_INSERTED) {
            if !self.init_and_insert(p) {
                return false;
            }
            h.set_prop_usage(p, usage::BUILDING);
        }
        match h.building_props(building) {
            BuildingProps::Present(mut v) => {
                if !v.contains(&p) {
                    v.push(p);
                    h.set_building_props(building, &v);
                }
            }
            BuildingProps::Absent => h.add_building_props_property(building, &[p]),
            // [inferred] No property holder: the property step is skipped.
            BuildingProps::NoHolder => {}
        }
        h.set_prop_usage(p, usage::BUILDING);
        true
    }

    // ---- queries (§A7.2) ----

    fn append(list: &mut Vec<u64>, found: Option<Vec<u64>>) -> bool {
        match found {
            Some(v) => {
                list.extend(v);
                true
            }
            None => false,
        }
    }

    /// Slot 17 `GetCityProps(list, SC4Rect<float>)`.
    pub fn get_city_props_world(&self, list: &mut Vec<u64>, rect: WorldRect) -> bool {
        if !self.has_occupant_manager() {
            return false;
        }
        self.set_filter_usage(usage::CITY);
        let (x1, z1, x2, z2) = rect;
        let found = self
            .host
            .occupants_in_box([x1, -f32::MAX, z1], [x2, f32::MAX, z2], usage::CITY);
        Self::append(list, found)
    }

    /// Slot 18 `GetCityProps(list, SC4Rect<int32>)`.
    pub fn get_city_props_cells(&self, list: &mut Vec<u64>, rect: CellRect) -> bool {
        self.cells_query(list, rect, usage::CITY)
    }

    /// Slot 30 `GetStreetProps(list, SC4Rect<int32>)`.
    pub fn get_street_props_cells(&self, list: &mut Vec<u64>, rect: CellRect) -> bool {
        self.cells_query(list, rect, usage::STREET)
    }

    fn cells_query(&self, list: &mut Vec<u64>, rect: CellRect, u: u32) -> bool {
        if !self.has_occupant_manager() {
            return false;
        }
        self.set_filter_usage(u);
        let found = self.host.occupants_in_cell_rect(rect, u);
        Self::append(list, found)
    }

    /// Slot 23 `GetLotProps(list, x, z)`.
    pub fn get_lot_props_at(&self, list: &mut Vec<u64>, x: i32, z: i32) -> bool {
        if self.s.borrow().lot_manager.is_none() {
            return false;
        }
        match self.host.lot_at_cell(x, z) {
            Some(lot) => self.get_lot_props(list, Some(lot)),
            None => false,
        }
    }

    /// Slot 24 `GetLotProps(list, lot)` (§A7.2): filters the whole list,
    /// including what was there before (quirk 7).
    pub fn get_lot_props(&self, list: &mut Vec<u64>, lot: Option<u64>) -> bool {
        let h = &*self.host;
        let Some(lot) = lot else {
            return false;
        };
        if !self.has_occupant_manager() {
            return false;
        }
        let (x1, z1, x2, z2) = h.lot_cell_rect(lot);
        self.set_filter_usage(usage::LOT);
        if !Self::append(list, h.occupants_in_cell_rect((x1, z1, x2, z2), usage::LOT)) {
            return false;
        }
        let (wx1, wz1) = h.cell_to_world(x1, z1);
        let (wx2, wz2) = h.cell_to_world(x2.wrapping_add(1), z2.wrapping_add(1));
        let mut kept = Vec::with_capacity(list.len());
        for occ in std::mem::take(list) {
            let p = h.position(occ);
            // A NaN coordinate fails every comparison.
            if wx1 <= p[0] && p[0] < wx2 && wz1 <= p[2] && p[2] < wz2 {
                kept.push(occ);
            } else {
                h.release(occ);
            }
        }
        *list = kept;
        true
    }

    /// Slot 31 `GetStreetProps(list, x, z)`: always false (quirk 4).
    pub fn get_street_props_at(&self, list: &mut Vec<u64>, x: i32, z: i32) -> bool {
        let h = &*self.host;
        if !self.has_occupant_manager() {
            return false;
        }
        self.set_filter_usage(usage::STREET);
        if Self::append(list, h.occupants_in_cell_rect((x, z, x, z), usage::STREET)) {
            let mut kept = Vec::with_capacity(list.len());
            for occ in std::mem::take(list) {
                let p = h.position(occ);
                if h.world_to_cell(p[0], p[2]) == (x, z) {
                    kept.push(occ);
                } else {
                    h.release(occ);
                }
            }
            *list = kept;
        }
        false
    }

    /// Slot 34 `GetBuildingProps(list, x, z)`.
    pub fn get_building_props_at(&self, list: &mut Vec<u64>, x: i32, z: i32) -> bool {
        if !self.has_occupant_manager() {
            return false;
        }
        let Some(b) = self.host.building_at_cell(x, z) else {
            return false;
        };
        let r = self.get_building_props(list, Some(b));
        self.host.release(b);
        r
    }

    /// Slot 35 `GetBuildingProps(list, building)`: every prop of the
    /// building's vector, in order, with a reference each.
    pub fn get_building_props(&self, list: &mut Vec<u64>, building: Option<u64>) -> bool {
        let Some(b) = building else {
            return false;
        };
        match self.host.building_props(b) {
            BuildingProps::Present(v) => {
                for p in v {
                    self.host.add_ref(p);
                    list.push(p);
                }
                true
            }
            _ => false,
        }
    }

    // ---- removing (§A7.3) ----

    /// Removing one prop (`0x004A2690`).
    fn remove_one(&self, occ: u64) {
        let h = &*self.host;
        if h.occupant_flag(occ, FLAG_SHOWN) {
            h.hide(occ);
        }
        if self.has_occupant_manager() && h.occupant_flag(occ, FLAG_INSERTED) {
            h.remove_occupant(occ);
        }
        h.occupant_shutdown(occ);
    }

    fn remove_list(&self, list: Vec<u64>) {
        for occ in list {
            self.remove_one(occ);
            self.host.release(occ);
        }
    }

    /// Slot 21 `RemoveCityProps(SC4Rect<float>)`. True.
    pub fn remove_city_props_world(&self, rect: WorldRect) -> bool {
        let mut l = Vec::new();
        self.get_city_props_world(&mut l, rect);
        self.remove_list(l);
        true
    }

    /// Slot 22 `RemoveCityProps(SC4Rect<int32>)`. True.
    pub fn remove_city_props_cells(&self, rect: CellRect) -> bool {
        let mut l = Vec::new();
        self.get_city_props_cells(&mut l, rect);
        self.remove_list(l);
        true
    }

    /// Slot 27 `RemoveLotProps(x, z)`. True.
    pub fn remove_lot_props_at(&self, x: i32, z: i32) -> bool {
        let mut l = Vec::new();
        self.get_lot_props_at(&mut l, x, z);
        self.remove_list(l);
        true
    }

    /// Slot 28 `RemoveLotProps(lot)`. True.
    pub fn remove_lot_props(&self, lot: Option<u64>) -> bool {
        let mut l = Vec::new();
        self.get_lot_props(&mut l, lot);
        self.remove_list(l);
        true
    }

    /// Slot 29 `RemoveProp(occ)` (SDK `RemovePropA`). True.
    pub fn remove_prop(&self, occ: u64) -> bool {
        self.remove_one(occ);
        true
    }

    /// Slot 33 `RemoveStreetPropsFromTile(x, z)`.
    pub fn remove_street_props_from_tile(&self, x: i32, z: i32) {
        let mut l = Vec::new();
        self.get_street_props_at(&mut l, x, z);
        self.remove_list(l);
    }

    /// Slot 37 `RemoveBuildingProps(x, z)`.
    pub fn remove_building_props_at(&self, x: i32, z: i32) -> bool {
        if !self.has_occupant_manager() {
            return false;
        }
        let Some(b) = self.host.building_at_cell(x, z) else {
            return false;
        };
        let r = self.remove_building_props(Some(b));
        self.host.release(b);
        r
    }

    /// RemoveOccupant (when the occupant manager exists and flag 1 is set),
    /// then `Shutdown`: the building-prop removal (no hiding).
    fn remove_building_member(&self, p: u64) {
        let h = &*self.host;
        if self.has_occupant_manager() && h.occupant_flag(p, FLAG_INSERTED) {
            h.remove_occupant(p);
        }
        h.occupant_shutdown(p);
    }

    /// Slot 38 `RemoveBuildingProps(building)`.
    pub fn remove_building_props(&self, building: Option<u64>) -> bool {
        let Some(b) = building else {
            return false;
        };
        let BuildingProps::Present(v) = self.host.building_props(b) else {
            return false;
        };
        for p in v {
            self.remove_building_member(p);
        }
        self.host.set_building_props(b, &[]);
        self.host.remove_building_props_property(b);
        true
    }

    /// Slot 39 `RemoveBuildingProp(building, prop)`: whether it was found.
    pub fn remove_building_prop(&self, building: Option<u64>, p: u64) -> bool {
        let Some(b) = building else {
            return false;
        };
        let BuildingProps::Present(mut v) = self.host.building_props(b) else {
            return false;
        };
        let Some(i) = v.iter().position(|&x| x == p) else {
            return false;
        };
        self.remove_building_member(p);
        v.remove(i);
        self.host.set_building_props(b, &v);
        if v.is_empty() {
            self.host.remove_building_props_property(b);
        }
        true
    }

    // ---- the record (§A8) ----

    fn parse(&self, inp: &mut dyn InStream, acquired: &mut Vec<u64>) -> Option<(Option<u64>, Option<u64>)> {
        if inp.get_u16()? != RECORD_VERSION {
            return None;
        }
        let mut read = |iid: u32| -> Option<Option<u64>> {
            let r = inp.get_ref_as(iid)?;
            let t = if r.is_null() {
                None
            } else {
                self.host.resolve_ref(&r, iid)
            };
            if let Some(t) = t {
                acquired.push(t);
            }
            Some(t)
        };
        let om = read(IID_OCCUPANT_MANAGER)?;
        let lm = read(IID_LOT_MANAGER)?;
        if inp.error() != 0 {
            return None;
        }
        Some((om, lm))
    }
}

impl SaveObject for PropManager {
    fn class_id(&self) -> u32 {
        CLSID_PROP_MANAGER
    }

    /// §A8: version 1, the occupant manager and the lot manager. True when
    /// the stream has no error.
    fn write(&self, out: &mut dyn OutStream) -> bool {
        let (om, lm) = {
            let s = self.s.borrow();
            (s.occupant_manager, s.lot_manager)
        };
        let r = |t: Option<u64>| t.map_or(RefValue::Null, |t| self.host.object_ref(t));
        out.put_u16(RECORD_VERSION);
        out.put_ref(&r(om));
        out.put_ref(&r(lm));
        out.error() == 0
    }

    /// §A8: false for any other version. [choice] Atomic.
    fn read(&self, inp: &mut dyn InStream) -> bool {
        let mut acquired = Vec::new();
        match self.parse(inp, &mut acquired) {
            Some((om, lm)) => {
                let old = {
                    let mut s = self.s.borrow_mut();
                    [
                        std::mem::replace(&mut s.occupant_manager, om),
                        std::mem::replace(&mut s.lot_manager, lm),
                    ]
                };
                for t in old.into_iter().flatten() {
                    self.host.release(t);
                }
                true
            }
            None => {
                for t in acquired {
                    self.host.release(t);
                }
                false
            }
        }
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }

    fn host_token(&self) -> u64 {
        self.token.get()
    }
}
