// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Runner for the language-neutral conformance vectors in
//! `engine/conformance/vectors/*.json`. The format is documented in
//! `engine/conformance/README.md`; this crate is one implementation of it,
//! running the vectors against `fc4-core`.

pub(crate) mod advisors;
pub(crate) mod audio;
pub(crate) mod aura;
pub(crate) mod automata;
pub(crate) mod budget;
pub(crate) mod city_detail;
pub(crate) mod city_situation;
pub(crate) mod civic;
pub(crate) mod clock24;
pub(crate) mod commercial;
pub(crate) mod crime;
pub(crate) mod demand;
pub(crate) mod demolition;
pub(crate) mod development;
pub(crate) mod development_simulator;
pub(crate) mod disasters;
pub(crate) mod dispatch;
pub(crate) mod effects;
pub(crate) mod flammability;
pub(crate) mod flora;
pub mod foundations;
pub(crate) mod grid;
pub(crate) mod growth_developer;
pub(crate) mod history;
pub mod json;
pub(crate) mod landfill;
pub(crate) mod landvalue;
pub(crate) mod lot_developer;
pub(crate) mod mysims;
pub(crate) mod neighbors;
pub(crate) mod network_bridge;
pub(crate) mod network_geometry;
pub(crate) mod network_manager;
pub(crate) mod network_rules;
pub(crate) mod occupant_manager;
pub(crate) mod ordinance;
pub(crate) mod parks;
pub(crate) mod pathfinder;
pub(crate) mod plumbing;
pub(crate) mod pollution;
pub(crate) mod port_developer;
pub(crate) mod power;
pub(crate) mod prop_developer;
pub(crate) mod prop_lotconfig;
pub(crate) mod prop_occupant;
pub(crate) mod protection;
pub(crate) mod residential;
pub mod runner;
pub(crate) mod scalar;
pub(crate) mod simgrid;
pub(crate) mod terrain;
pub(crate) mod tract_developer;
pub(crate) mod traffic_network;
pub(crate) mod traffic_simulator;
pub(crate) mod tutorial;
pub(crate) mod ui_windows;
pub mod variants;
pub(crate) mod weather;
pub(crate) mod zones;
pub(crate) mod dates;

pub use runner::{run_vector_file, run_vector_text};
