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

//! Readers for SimCity 4 game data.
//!
//! - [`dbpf`]: the DBPF 1.x package container (`.dat`, `.sc4`, `.sc4lot`, ...), index 7.0 and 7.1, the DIR
//!   record of compressed entries, random-access entry reads from a file or any byte buffer (for example a
//!   memory map the caller owns), and a small package writer.
//! - [`qfs`]: QFS/RefPack decompression and compression with the 9-byte SimCity 4 header.
//! - [`exemplar`]: exemplars and cohorts in binary (`EQZB`) and text (`EQZT`) form as the original's factories
//!   read them, the property getters with their type coercion, and a store that loads records with their parent
//!   cohorts (`docs/specs/data-formats.md` §4, §5).
//! - [`ltext`]: localized text (LTEXT) entries.
//! - [`fsh`]: FSH (`SHPI`) textures as the original's factory accepts them, their mipmap levels, and a CPU decoder
//!   to RGBA8 (`docs/specs/graphics-formats.md` §2).
//! - [`s3d`]: S3D (`3DMD`) models with the chunk grammar of each version, materials, animation frames and the
//!   meshes of a frame (graphics-formats §3).
//! - [`keys`]: how a model's material names its texture and how an exemplar names its model (graphics-formats §4).
//!
//! Written from public format documentation (see `README.md`), then brought in line with
//! `docs/specs/data-formats.md` and `docs/specs/foundations.md` (§2.3, §3.1, §9.2, and §18 for the x87
//! arithmetic of the getters and the text parser).
//! Places where the documentation leaves a decision open are tagged `\[choice\]`. Nothing here panics on bad
//! input: every parser returns a [`Result`].

#![forbid(unsafe_code)]

pub mod city_save;
pub mod command_values;
pub mod dbpf;
pub mod effects;
pub mod error;
pub mod exemplar;
pub mod fsh;
pub mod keys;
pub mod ltext;
pub mod qfs;
mod reader;
pub mod s3d;
pub mod tgi;
mod x87;

pub use dbpf::{FileSource, IndexEntry, MemSource, Package, PackageBuilder, Source};
pub use error::{Error, Result};
pub use exemplar::{Exemplar, ExemplarStore, LoadError, LoadedExemplar, Property, ValueType, Values};
pub use tgi::Tgi;
