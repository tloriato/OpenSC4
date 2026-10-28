// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Version-two demand-simulator records.

use super::*;

game_fn! {
    fn write_serializable(stream: *mut u8, object: *mut u8) -> u32 = "C" 0x005D_99A0;
}

port! {
    /// Write four float words, the ordered demand references, and the ordered previous census.
    /// `this` is the serializable interface, eight bytes after the primary interface.
    /// The entry exposes the stream argument reused as temporary census-value storage.
    #[port(address = 0x006A_2680, owner = "demand", name = "cSC4DemandSimulator::Write", naked)]
    pub unsafe extern "C" fn fc4p_006a2680() {
        core::arch::naked_asm!(
            "push edx", "push eax", "mov edx, esp", "call {body}", "add esp, 8", "ret 4",
            body = sym write,
        )
    }
}

unsafe extern "fastcall" fn write(this: *mut u8, entry: *mut Entry) -> u64 {
    unsafe {
        let entry = &mut *entry;
        let sim = DemandSim::from_ptr(this.sub(8));
        let stream = entry.arg(0) as *mut u8;
        let mut result = virtual_call(stream, 0x48, read_unaligned(stream.cast()), entry.edx, &[]);
        if result.eax as u8 != 0 {
            return registers(result.eax & !0xFF, result.edx);
        }
        result = virtual_call(stream, 0x1C, result.eax, read_unaligned(stream.cast()), &[2]);
        virtual_call(
            stream,
            0x30,
            read_unaligned(stream.cast()),
            result.edx,
            &[sim.commercial_bonus()],
        );
        for offset in [0xA4, 0xA8, 0xAC] {
            let word = read_unaligned(sim.as_ptr().add(offset).cast());
            virtual_call(stream, 0x30, word, read_unaligned(stream.cast()), &[word]);
        }
        let count = sim.node_count();
        virtual_call(stream, 0x24, count, read_unaligned(stream.cast()), &[count]);
        let mut node = TreeNode::from_ptr(sim.nodes()).left();
        while node != sim.nodes() {
            let view = TreeNode::from_ptr(node);
            let object = view.value() as *mut u8;
            let key = view.key();
            virtual_call(stream, 0x24, key, read_unaligned(stream.cast()), &[key]);
            write_serializable(stream, object);
            node = successor(node);
        }
        let count = sim.previous_census_count();
        virtual_call(stream, 0x24, count, read_unaligned(stream.cast()), &[count]);
        let mut header = sim.previous_census();
        node = TreeNode::from_ptr(header).left();
        while node != header {
            let view = TreeNode::from_ptr(node);
            entry.put(0, view.value());
            let key = view.key();
            virtual_call(stream, 0x24, key, read_unaligned(stream.cast()), &[key]);
            let value = entry.arg(0);
            virtual_call(stream, 0x30, value, read_unaligned(stream.cast()), &[value]);
            node = successor(node);
            header = sim.previous_census();
        }
        result = virtual_call(stream, 0x48, header as u32, read_unaligned(stream.cast()), &[]);
        registers((result.eax & !0xFF) | u32::from(result.eax as u8 == 0), result.edx)
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_006a2680::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demand::interface::tests::Map;
    use crate::testing::{call_thiscall_with_eax, fake, Object};

    #[derive(Debug, PartialEq)]
    enum Call {
        Error,
        Version(u32),
        Float(u32),
        Word(u32),
        Reference(u32),
    }
    #[repr(C)]
    struct Stream {
        table: *const usize,
        bytes: Vec<u8>,
        calls: Vec<Call>,
        initial_error: u32,
        final_error: u32,
        // A callee changes a later field to check when the writer reads it.
        simulator: *mut u8,
        mutate: bool,
    }
    extern "thiscall" fn error(this: *mut Stream) -> u64 {
        unsafe {
            let s = &mut *this;
            let value = if s.calls.is_empty() {
                s.initial_error
            } else {
                s.final_error
            };
            s.calls.push(Call::Error);
            registers(value, 0x8765_4321)
        }
    }
    extern "thiscall" fn version(this: *mut Stream, value: u32) -> u64 {
        unsafe {
            (*this).calls.push(Call::Version(value));
            (*this).bytes.extend_from_slice(&(value as u16).to_le_bytes());
        }
        0
    }
    extern "thiscall" fn float(this: *mut Stream, value: u32) -> u64 {
        unsafe {
            let s = &mut *this;
            s.calls.push(Call::Float(value));
            s.bytes.extend_from_slice(&value.to_le_bytes());
            if s.mutate {
                DemandSim::from_ptr(s.simulator).set_industrial_relief(0xFF80_0000);
            }
        }
        0
    }
    extern "thiscall" fn word(this: *mut Stream, value: u32) -> u64 {
        unsafe {
            (*this).calls.push(Call::Word(value));
            (*this).bytes.extend_from_slice(&value.to_le_bytes());
        }
        0
    }
    extern "C" fn reference(this: *mut Stream, value: u32) -> u32 {
        unsafe {
            (*this).calls.push(Call::Reference(value));
            (*this).bytes.extend_from_slice(&value.to_le_bytes());
            (*this).bytes.extend_from_slice(&0x8990_C12Du32.to_le_bytes());
        }
        0
    }

    #[test]
    fn saved_record_preserves_float_bits_and_sorted_maps_and_final_error_registers() {
        let _fake = fake(0x005D_99A0, reference as *const () as usize);
        // The core save-round-trip case: two demand references, bonuses 0.5 and 0.25.
        for empty in [false, true] {
            let values = [(0x1010, 1), (0x2010, 2)];
            let census_values = [(0x1011, 1234.5f32.to_bits()), (0x3111, 0x7FA1_2345)];
            let mut nodes = Map::new(if empty { &[] } else { &values });
            let mut census = Map::new(if empty { &[] } else { &census_values });
            let mut memory = Object::new(0xB0);
            let sim = unsafe { DemandSim::from_ptr(memory.ptr()) };
            sim.set_nodes(nodes.header());
            sim.set_node_count(if empty { 0 } else { 2 });
            sim.set_previous_census(census.header());
            sim.set_previous_census_count(if empty { 0 } else { 2 });
            sim.set_commercial_bonus(0.5f32.to_bits());
            sim.set_industrial_bonus(0.25f32.to_bits());
            sim.set_commercial_relief(0x8000_0000);
            sim.set_industrial_relief(0x7FC1_2345);
            let mut table = [0usize; 19];
            table[0x48 / 4] = error as *const () as usize;
            table[0x1C / 4] = version as *const () as usize;
            table[0x30 / 4] = float as *const () as usize;
            table[0x24 / 4] = word as *const () as usize;
            for final_error in [0xAABB_CC00, 0xAABB_CC01, 0xAABB_CCFF] {
                let mut stream = Stream {
                    table: table.as_ptr(),
                    bytes: Vec::new(),
                    calls: Vec::new(),
                    initial_error: 0,
                    final_error,
                    simulator: sim.as_ptr(),
                    mutate: false,
                };
                let result = unsafe {
                    call_thiscall_with_eax(
                        fc4p_006a2680 as *const () as usize,
                        sim.as_ptr().add(8),
                        &[&mut stream as *mut Stream as u32],
                        0,
                    )
                };
                assert_eq!(
                    result,
                    ((final_error & !0xFF) | u32::from(final_error as u8 == 0), 0x8765_4321)
                );
                let mut expected = vec![
                    Call::Error,
                    Call::Version(2),
                    Call::Float(0.5f32.to_bits()),
                    Call::Float(0.25f32.to_bits()),
                    Call::Float(0x8000_0000),
                    Call::Float(0x7FC1_2345),
                    Call::Word(if empty { 0 } else { 2 }),
                ];
                if !empty {
                    expected.extend([
                        Call::Word(0x1010),
                        Call::Reference(1),
                        Call::Word(0x2010),
                        Call::Reference(2),
                    ]);
                }
                expected.push(Call::Word(if empty { 0 } else { 2 }));
                if !empty {
                    expected.extend([
                        Call::Word(0x1011),
                        Call::Float(1234.5f32.to_bits()),
                        Call::Word(0x3111),
                        Call::Float(0x7FA1_2345),
                    ]);
                }
                expected.push(Call::Error);
                assert_eq!(stream.calls, expected);
                let mut bytes = vec![2, 0];
                for call in &expected {
                    match *call {
                        Call::Float(v) | Call::Word(v) => bytes.extend_from_slice(&v.to_le_bytes()),
                        Call::Reference(v) => {
                            bytes.extend_from_slice(&v.to_le_bytes());
                            bytes.extend_from_slice(&0x8990_C12Du32.to_le_bytes());
                        }
                        _ => {}
                    }
                }
                assert_eq!(stream.bytes, bytes);
            }
        }
    }

    #[test]
    fn initial_stream_error_writes_nothing_and_retains_upper_eax_and_edx() {
        let mut table = [0usize; 19];
        table[0x48 / 4] = error as *const () as usize;
        let mut memory = Object::new(0xB0);
        let mut stream = Stream {
            table: table.as_ptr(),
            bytes: Vec::new(),
            calls: Vec::new(),
            initial_error: 0xAABB_CCFF,
            final_error: 0,
            simulator: memory.ptr(),
            mutate: false,
        };
        let result = unsafe {
            call_thiscall_with_eax(
                fc4p_006a2680 as *const () as usize,
                memory.ptr().add(8),
                &[&mut stream as *mut Stream as u32],
                0,
            )
        };
        assert_eq!(result, (0xAABB_CC00, 0x8765_4321));
        assert_eq!(stream.calls, [Call::Error]);
        assert!(stream.bytes.is_empty());
    }

    #[test]
    fn writer_reads_later_fields_after_calls_and_retains_census_scratch_word() {
        let mut table = [0usize; 19];
        table[0x48 / 4] = error as *const () as usize;
        table[0x1C / 4] = version as *const () as usize;
        table[0x30 / 4] = float as *const () as usize;
        table[0x24 / 4] = word as *const () as usize;
        let mut nodes = Map::new(&[]);
        let mut census = Map::new(&[(1, 0x7FA1_2345)]);
        let mut memory = Object::new(0xB0);
        let sim = unsafe { DemandSim::from_ptr(memory.ptr()) };
        sim.set_nodes(nodes.header());
        sim.set_previous_census(census.header());
        sim.set_previous_census_count(1);
        let mut stream = Stream {
            table: table.as_ptr(),
            bytes: Vec::new(),
            calls: Vec::new(),
            initial_error: 0,
            final_error: 0,
            simulator: sim.as_ptr(),
            mutate: true,
        };
        let mut probe = crate::demand::test_support::Probe::new(
            fc4p_006a2680 as *const (),
            unsafe { sim.as_ptr().add(8) },
            [&mut stream as *mut Stream as u32, 0xABCD, 0xDCBA],
        );
        unsafe {
            probe.run();
        }
        assert_eq!(
            stream.calls,
            [
                Call::Error,
                Call::Version(2),
                Call::Float(0),
                Call::Float(0),
                Call::Float(0),
                Call::Float(0xFF80_0000),
                Call::Word(0),
                Call::Word(1),
                Call::Word(1),
                Call::Float(0x7FA1_2345),
                Call::Error
            ]
        );
        assert_eq!(probe.after, [0x7FA1_2345, 0xABCD, 0xDCBA]);
    }
}
