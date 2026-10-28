// SPDX-License-Identifier: GPL-3.0-or-later
//! Package wiring for terrain.md §13.1. Layouts remain in the terrain owner.
use crate::{Package, Source, Tgi};
use fc4_core::terrain::{Terrain, TerrainTuning};
use std::rc::Rc;

pub(super) const MISC: Tgi = Tgi::new(0x8A91_E7E3, 0xE98F_9525, 3);
pub(super) const ALTITUDES: Tgi = Tgi::new(0xA9DD_6FF4, 0xE98F_9525, 1);
pub(super) const PROPERTIES: Tgi = Tgi::new(0x8A91_E7E0, 0xE98F_9525, 2);

#[derive(Clone, Copy)]
pub(super) enum Part {
    Misc,
    Altitudes,
    Properties,
}

impl Part {
    pub fn from_key(key: Tgi) -> Option<Self> {
        match key {
            MISC => Some(Self::Misc),
            ALTITUDES => Some(Self::Altitudes),
            PROPERTIES => Some(Self::Properties),
            _ => None,
        }
    }

    pub fn write(self, terrain: &Terrain) -> Vec<u8> {
        match self {
            Self::Misc => terrain.write_misc(),
            Self::Altitudes => terrain.write_altitudes(),
            Self::Properties => terrain.write_properties(),
        }
    }
}

pub(super) fn load<S: Source>(package: &Package<S>) -> Result<Option<Rc<Terrain>>, String> {
    if ![MISC, ALTITUDES, PROPERTIES]
        .iter()
        .any(|key| package.find(key).is_some())
    {
        return Ok(None);
    }
    let read = |key| package.read_tgi(&key).map_err(|e| e.to_string());
    Terrain::load(
        &read(MISC)?,
        &read(ALTITUDES)?,
        &read(PROPERTIES)?,
        TerrainTuning::default(),
    )
    .map(|t| Some(Rc::new(t)))
    .map_err(|e| format!("terrain Load: {e:?}"))
}

pub(super) fn is_edge(key: Tgi) -> bool {
    key.type_id == ALTITUDES.type_id && key.group == ALTITUDES.group && (0x64..=0x67).contains(&key.instance)
}
