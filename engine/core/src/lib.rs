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

//! Portable FreeCity4 simulation core.
//!
//! This crate contains no GZCOM or Windows types. It is written only from the
//! clean-room behaviour specs in `docs/specs/` (currently
//! `docs/specs/simulator.md`). Section references (`§n`) in comments point at
//! that spec. Places where the spec left a decision open are tagged `[choice]`.

#![forbid(unsafe_code)]

pub mod advisors;
pub mod audio;
pub mod aura;
pub mod automata;
pub mod budget;
pub mod calibration;
pub mod city_detail;
pub mod city_situation;
pub mod civic;
pub mod clock24;
pub mod commercial;
pub mod crime;
pub mod curve;
pub mod demand;
pub mod demolition;
pub mod development;
pub mod development_simulator;
pub mod disasters;
pub mod dispatch;
pub mod effects;
pub mod flammability;
pub mod flora;
pub mod grid;
pub mod growth_developer;
pub mod gz_messages;
pub mod history;
pub mod host;
pub mod ids;
pub mod julian;
pub mod landfill;
pub mod landvalue;
pub mod lot_config_manager;
pub mod lot_developer;
pub mod lot_records;
pub mod mysims;
pub mod neighbors;
pub mod network_bridge;
pub mod network_geometry;
pub mod network_manager;
pub mod network_occupants;
pub mod network_rules;
pub mod network_tools;
pub mod occupant_manager;
pub mod ordinance;
pub mod pacing;
pub mod parks;
pub mod pathfinder;
pub mod plumbing;
pub mod pollution;
pub mod port_developer;
pub mod power;
pub mod prop_developer;
pub mod prop_manager;
pub mod prop_occupant;
pub mod property;
pub mod props;
pub mod protection;
pub mod residential;
pub mod rng;
pub mod save;
pub mod seeding;
pub mod simgrid;
pub mod simulator;
pub mod span_render4444;
pub mod span_render1555;pub mod stl;
pub mod span_render565;
pub mod stl;pub mod store;
pub mod stream;
pub mod terrain;
pub mod tract_developer;
pub mod traffic_network;
pub mod traffic_simulator;
pub mod tutorial;
pub mod ui_windows;
pub mod variant;
pub mod weather;
pub mod x87;
pub mod zones;

pub use calibration::Calibration;
pub use host::{AgentDelivery, AgentHandle, Broadcast, Clock, Host, Message, TimerSource};
pub use save::{LoadError, SaveRecord};
pub use simulator::{AgentInfo, DebugInfo, PacingHooks, Simulator, Snapshot, Tuning, Turbo, TURBO_DEFAULT_FRAME_MS};
