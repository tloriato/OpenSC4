// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The converting getters the command system calls on parameter values
//! (`GetValString`, `GetValSint32`; docs/specs/data-formats.md §5.1), for
//! `fc4_core::commands` (docs/specs/command-system.md §1.4, §3.6, §4.4).
//!
//! A core variant is mapped onto an exemplar [`Property`] value, whose getters
//! implement §5.1 (text round trip, float printing, number parsing).

use fc4_core::commands::ValueConvert;
use fc4_core::variant::{mem, Elem, Variant};

use crate::exemplar::{Property, Values};

/// The exact converter: [`ValueConvert`] through [`Property`]'s getters.
#[derive(Clone, Copy, Debug, Default)]
pub struct PropertyConvert;

fn values_of(elem: u16, items: &[Elem]) -> Option<Values> {
    macro_rules! collect {
        ($variant:ident, $pat:ident) => {
            Values::$variant(
                items
                    .iter()
                    .filter_map(|e| if let Elem::$pat(v) = *e { Some(v) } else { None })
                    .collect(),
            )
        };
    }
    Some(match elem {
        mem::BOOL => collect!(Bool, Bool),
        mem::UINT8 => collect!(Uint8, U8),
        mem::SINT8 => collect!(Sint8, I8),
        mem::UINT16 => collect!(Uint16, U16),
        mem::SINT16 => collect!(Sint16, I16),
        mem::UINT32 => collect!(Uint32, U32),
        mem::SINT32 => collect!(Sint32, I32),
        mem::UINT64 => collect!(Uint64, U64),
        mem::SINT64 => collect!(Sint64, I64),
        mem::FLOAT32 => collect!(Float32, F32),
        mem::FLOAT64 => collect!(Float64, F64),
        mem::RZ_CHAR => collect!(String, RzChar),
        mem::CHAR => collect!(Char, Char),
        mem::UNICODE_CHAR => collect!(UnicodeChar, UnicodeChar),
        // The void array; a VoidPtr array also has the empty text (data-formats §5.1).
        0 => collect!(Void, U8),
        mem::VOID_PTR => Values::Void(Vec::new()),
        _ => return None,
    })
}

/// The value as a property; `None` for a value without text (an object: data-formats §5.1
/// *[choice]*, a missing property; a VoidPtr scalar: [choice] GC7, no text either).
fn property_of(v: &Variant) -> Option<Property> {
    match v {
        Variant::Empty => Some(Property::single(0, Values::Empty)),
        Variant::Object(_) => None,
        Variant::Scalar(Elem::VoidPtr(_)) => None,
        Variant::Scalar(e) => Some(Property::single(0, values_of(e.mem_type(), &[*e])?)),
        Variant::Array { elem, items } => Some(Property::array(0, values_of(*elem, items)?)),
    }
}

impl ValueConvert for PropertyConvert {
    fn get_val_string(&self, v: &Variant, out: &mut Vec<u8>) -> bool {
        match property_of(v) {
            Some(p) => p.get_string_into(out),
            None => false,
        }
    }

    fn get_val_sint32(&self, v: &Variant) -> Option<i32> {
        property_of(v)?.get_i32()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fc4_core::commands::{CommandParameterSet, StringCommandGenerator, STATUS_PARAMETER_ID};

    fn text(v: &Variant) -> (bool, Vec<u8>) {
        let mut out = b"old".to_vec();
        let ok = PropertyConvert.get_val_string(v, &mut out);
        (ok, out)
    }

    #[test]
    fn get_val_string_by_type() {
        assert_eq!(text(&Variant::string(b"5")), (true, b"5".to_vec()));
        assert_eq!(text(&Variant::Scalar(Elem::I32(7))), (false, b"7".to_vec()));
        assert_eq!(text(&Variant::Scalar(Elem::U32(3))), (false, b"3".to_vec()));
        assert_eq!(text(&Variant::Scalar(Elem::F32(2.75))), (false, b"2.75".to_vec()));
        assert_eq!(text(&Variant::Empty), (false, Vec::new()));
        assert_eq!(text(&Variant::Scalar(Elem::VoidPtr(1))), (false, b"old".to_vec()));
        assert_eq!(
            text(&Variant::array(mem::UINT32, vec![Elem::U32(9), Elem::U32(4)])),
            (false, b"9".to_vec())
        );
    }

    #[test]
    fn get_val_sint32_by_type() {
        let c = PropertyConvert;
        assert_eq!(c.get_val_sint32(&Variant::string(b"7")), Some(7));
        assert_eq!(c.get_val_sint32(&Variant::Scalar(Elem::I32(-2))), Some(-2));
        assert_eq!(c.get_val_sint32(&Variant::Empty), Some(0));
        assert_eq!(c.get_val_sint32(&Variant::Scalar(Elem::F32(-1.5))), Some(-1));
    }

    /// command-system §9 vectors 7 and 12 with the exact converter.
    #[test]
    fn spec_vectors_7_and_12() {
        let mut p = CommandParameterSet::new();
        p.set_parameter(0, &Variant::string(b"ABC"));
        p.set_parameter(1, &Variant::Scalar(Elem::I32(5)));
        p.set_parameter(2, &Variant::string(b""));
        assert!(p.compare_parameter_string(0, b"abc", false, &PropertyConvert));
        assert!(!p.compare_parameter_string(1, b"5", false, &PropertyConvert));
        assert!(p.compare_parameter_string(2, b"", true, &PropertyConvert));
        let g = StringCommandGenerator::new(std::rc::Rc::new(PropertyConvert));
        let q = CommandParameterSet::new_ref();
        q.borrow_mut().set_parameter(0, &Variant::string(b"x y"));
        q.borrow_mut().set_parameter(1, &Variant::Scalar(Elem::U32(3)));
        q.borrow_mut()
            .set_parameter(STATUS_PARAMETER_ID, &Variant::Scalar(Elem::I32(0)));
        let mut out = Vec::new();
        assert!(g.convert_command_output_to_string(Some(&q), &mut out));
        assert_eq!(out, b"\"x y\" 3 0");
    }
}
