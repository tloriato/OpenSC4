// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline hosts: record Read/Write do not need a running city or clock.
use fc4_core::clock24::{ClockHost, ClockTuning};
use fc4_core::history::HistoryHost;
use std::rc::Rc;

pub(super) struct Offline;
impl HistoryHost for Offline {
    fn sim_date(&self) -> u32 {
        0
    }
    fn now_ms(&self) -> u32 {
        0
    }
    fn subscribe(&self, _: u32) {}
    fn unsubscribe(&self, _: u32) {}
    fn add_agent(&self, _: u32, _: &str, _: u32) {}
    fn remove_agent(&self) {}
    fn send(&self, _: u32, _: u32, _: u32) {}
}
impl ClockHost for Offline {
    fn now_micros(&self) -> u64 {
        0
    }
    fn post(&self, _: u32, _: u32, _: u32) {}
    fn add_system_service(&self) {}
    fn remove_system_service(&self) {}
    fn add_to_on_idle(&self) {}
    fn remove_from_on_idle(&self) {}
    fn has_cheat_manager(&self) -> bool {
        false
    }
    fn register_cheat(&self, _: u32, _: &str) {}
    fn add_cheat_notification(&self, _: u32) {}
    fn remove_cheat_notification(&self, _: u32) {}
    fn unregister_cheat(&self, _: u32) {}
    fn tuning(&self) -> Option<ClockTuning> {
        None
    }
}

impl fc4_core::weather::WeatherHost for Offline {
    fn city_size(&self) -> (f32, f32) {
        (0.0, 0.0)
    }
    fn city_cell_width(&self) -> (f32, f32) {
        (16.0, 16.0)
    }
    fn world_hemisphere(&self) -> i32 {
        1
    }
    fn sim_month(&self) -> i32 {
        1
    }
    fn water_prefs(&self) -> Option<(u8, u8)> {
        None
    }
    fn tuning(&self) -> Option<Rc<fc4_core::property::PropertyHolder>> {
        None
    }
    fn terrain_available(&self) -> bool {
        false
    }
    fn altitude_at_vertex(&self, _: i32, _: i32) -> f32 {
        0.0
    }
    fn vertex_normal(&self, _: i32, _: i32) -> [f32; 3] {
        [0.0, 1.0, 0.0]
    }
    fn cell_has_water(&self, _: i32, _: i32) -> bool {
        false
    }
    fn altitude(&self, _: f32, _: f32) -> f32 {
        0.0
    }
    fn sea_level(&self) -> f32 {
        0.0
    }
    fn post(&self, _: u32, _: u32) {}
}
