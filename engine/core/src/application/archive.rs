// SPDX-License-Identifier: GPL-3.0-or-later
//! Root and version records, foundations §3.2 and application §11.9.
use crate::store::archive::{fields::array, ObjectReference};
use crate::stream::{InStream, OutStream};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationRecord {
    Root(ObjectReference),
    SaveVersion(u32),
    ModuleVersion([u32; 4]),
}

impl ApplicationRecord {
    pub fn read(type_id: u32, r: &mut dyn InStream) -> Option<Self> {
        Some(match type_id {
            0x49DD_6F4C => {
                if r.get_u16()? != 1 || r.get_u16()? != 2 {
                    return None;
                }
                Self::Root(ObjectReference::read(r)?)
            }
            0xEA3F_F4F5 => Self::SaveVersion(r.get_u32()?),
            0x0B7D_45E0 => {
                if r.get_u16()? != 3 {
                    return None;
                }
                Self::ModuleVersion(array(r, |r| r.get_u32())?)
            }
            _ => return None,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        match self {
            Self::Root(city) => {
                w.put_u16(1);
                w.put_u16(2);
                city.write(w);
            }
            Self::SaveVersion(version) => {
                w.put_u32(*version);
            }
            Self::ModuleVersion(parts) => {
                w.put_u16(3);
                for &part in parts {
                    w.put_u32(part);
                }
            }
        }
        w.error() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::{ByteReader, ByteWriter};

    #[test]
    fn root_is_a_saved_reference_and_shortens_when_cleared() {
        let bytes = [1, 0, 2, 0, 17, 0, 0, 0, 0x72, 0xC3, 0x90, 0x89];
        let mut r = ByteReader::new(&bytes);
        let mut root = ApplicationRecord::read(0x49DD_6F4C, &mut r).unwrap();
        assert_eq!(r.remaining(), 0);
        assert_eq!(
            root,
            ApplicationRecord::Root(ObjectReference {
                id: 17,
                class: 0x8990_C372
            })
        );
        let mut w = ByteWriter::new();
        assert!(root.write(&mut w));
        assert_eq!(w.into_bytes(), bytes);
        root = ApplicationRecord::Root(ObjectReference::default());
        let mut w = ByteWriter::new();
        assert!(root.write(&mut w));
        assert_eq!(w.into_bytes(), [1, 0, 2, 0, 0, 0, 0, 0]);
        for end in 0..bytes.len() {
            assert!(ApplicationRecord::read(0x49DD_6F4C, &mut ByteReader::new(&bytes[..end])).is_none());
        }
    }
}
