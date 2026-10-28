// SPDX-License-Identifier: GPL-3.0-or-later
//! Port-developers §7. Keep the saved tuning key and load-order type index
//! without loading exemplars, looking up lots or recomputing sea access.
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug)]
pub struct SavedPort {
    pub stage: u32,
    pub type_index: u16,
    pub cell: [i16; 2],
}

#[derive(Clone, Debug)]
pub struct PortRecord {
    pub tuning_instance: u32,
    pub zone: u16,
    pub ports: Vec<SavedPort>,
}

impl PortRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        if r.get_u16()? != 2 || r.get_u16()? != 1 {
            return None;
        }
        let tuning_instance = r.get_u32()?;
        let zone = r.get_u16()?;
        let n = r.get_u16()? as usize;
        if n > r.remaining() / 10 {
            return None;
        }
        let mut ports = Vec::with_capacity(n);
        for _ in 0..n {
            ports.push(SavedPort {
                stage: r.get_u32()?,
                type_index: r.get_u16()?,
                cell: [r.get_i16()?, r.get_i16()?],
            });
        }
        Some(Self {
            tuning_instance,
            zone,
            ports,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        let Ok(n) = u16::try_from(self.ports.len()) else {
            return false;
        };
        w.put_u16(2);
        w.put_u16(1);
        w.put_u32(self.tuning_instance);
        w.put_u16(self.zone);
        w.put_u16(n);
        for port in &self.ports {
            w.put_u32(port.stage);
            w.put_u16(port.type_index);
            for cell in port.cell {
                w.put_i16(cell);
            }
        }
        w.error() == 0
    }
}
