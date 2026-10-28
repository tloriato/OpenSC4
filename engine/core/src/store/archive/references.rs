// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline reference streams (foundations §2.7). Tokens carry saved object IDs,
//! not host pointers. No object is initialized, visited, or newly registered;
//! cycles and references to owners not yet decoded keep their original IDs.

use crate::stream::{ByteWriter, OutStream, RefResolver, RefValue};

pub struct SavedReferences;

impl RefResolver for SavedReferences {
    fn resolve(&mut self, class_id: u32, object_id: u32) -> Option<RefValue> {
        Some(RefValue::Host {
            token: u64::from(object_id),
            class_id,
        })
    }
}

/// Writes the identities returned by `SavedReferences` without renumbering.
/// Deliberately rejects live objects and pointer-sized tokens: callers must not
/// accidentally mix a running city's reference registry with an archive.
#[derive(Default)]
pub struct ReferenceWriter(ByteWriter);

impl ReferenceWriter {
    pub fn into_bytes(self) -> Vec<u8> {
        self.0.into_bytes()
    }
}

macro_rules! forward {
    ($name:ident, $ty:ty) => {
        fn $name(&mut self, value: $ty) -> bool {
            self.0.$name(value)
        }
    };
}

impl OutStream for ReferenceWriter {
    forward!(put_u8, u8);
    forward!(put_u16, u16);
    forward!(put_u32, u32);
    forward!(put_u64, u64);
    forward!(put_f32, f32);
    forward!(put_f64, f64);
    forward!(put_bytes, &[u8]);
    forward!(put_str, &[u8]);

    fn put_ref(&mut self, value: &RefValue) -> bool {
        match value {
            RefValue::Null => self.0.put_u32(0),
            RefValue::Host { token, class_id } if *token != 0 && *token <= u64::from(u32::MAX) => {
                self.0.put_u32(*token as u32) && self.0.put_u32(*class_id)
            }
            _ => {
                self.0.set_error();
                false
            }
        }
    }

    fn error(&self) -> i32 {
        self.0.error()
    }

    fn set_error(&mut self) {
        self.0.set_error();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::{ByteReader, InStream};

    #[test]
    fn null_repeated_forward_and_class_scoped_identities_keep_their_encoding() {
        let mut original = ByteWriter::new();
        for (id, class) in [(0, 0), (19, 71), (u32::MAX, 93), (19, 71), (19, 93)] {
            original.put_u32(id);
            if id != 0 {
                original.put_u32(class);
            }
        }
        let bytes = original.into_bytes();
        let mut references = SavedReferences;
        let mut input = ByteReader::with_resolver(&bytes, &mut references);
        let mut output = ReferenceWriter::default();
        for _ in 0..5 {
            assert!(output.put_ref(&input.get_ref().unwrap()));
        }
        assert_eq!(input.remaining(), 0);
        assert_eq!(output.into_bytes(), bytes);
    }

    #[test]
    fn invalid_id_fails_without_truncation_and_stays_failed() {
        for token in [0, u64::from(u32::MAX) + 1] {
            let mut output = ReferenceWriter::default();
            assert!(!output.put_ref(&RefValue::Host { token, class_id: 17 }));
            assert_ne!(output.error(), 0);
            assert!(!output.put_u32(1));
            assert!(output.into_bytes().is_empty());
        }
    }
}
