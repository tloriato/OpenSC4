// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Jobs-per-unit factors from the demand exemplar's drives property.

use super::*;
use crate::x87::F80;

game_data! { static resource_manager: *mut u8 = 0x00B4_3CA8; }
game_data! { static unsigned_bias: f32 = 0x00A8_0AA8; }
game_data! { static percent_scale: f32 = 0x00A8_23A4; }

port! {
    /// Fill jobs[1..=3] from matching drive links; jobs[0] is untouched. Missing resources leave three zeros.
    /// The entry exposes the argument words reused for the resource pointer and element count.
    #[port(address = 0x006A_23A0, owner = "demand", name = "cSC4DemandSimulator::CalculateJobsPerUnitOfDemand", naked)]
    pub unsafe extern "C" fn fc4p_006a23a0() {
        core::arch::naked_asm!(
            "push edx", "push eax", "mov edx, esp", "call {body}", "add esp, 8", "ret 12",
            body = sym calculate_jobs,
        )
    }
}

unsafe extern "fastcall" fn calculate_jobs(_this: *mut u8, entry: *mut Entry) -> u64 {
    unsafe {
        let entry = &mut *entry;
        let output = entry.arg(0) as *mut u32;
        let index = entry.arg(2);
        let key = [0x6534_284A, 0xC7BB_4816, entry.arg(1).wrapping_add(index)];
        for i in 1..=3 {
            write_unaligned(output.add(i), 0);
        }
        entry.put(0, 0);
        let manager = read_unaligned(resource_manager());
        let mut result = virtual_call(
            manager,
            0x0C,
            key.as_ptr() as u32,
            read_unaligned(manager.cast()),
            &[key.as_ptr() as u32, 0xA521_60F5, entry.args.as_mut_ptr() as u32, 0, 0],
        );
        if result.eax as u8 != 0 {
            let resource = entry.arg(0) as *mut u8;
            result = virtual_call(resource, 0x1C, result.eax, read_unaligned(resource.cast()), &[]);
            if result.eax != 0 {
                let holder = result.eax as *mut u8;
                result = virtual_call(
                    holder,
                    0x24,
                    holder as u32,
                    read_unaligned(holder.cast()),
                    &[0x47BB_3F30],
                );
                if result.eax != 0 {
                    let property = result.eax as *mut u8;
                    result = virtual_call(property, 0x14, property as u32, read_unaligned(property.cast()), &[]);
                    let variant = result.eax as *mut u8;
                    result = virtual_call(variant, 0x19C, read_unaligned(variant.cast()), result.edx, &[]);
                    let data = result.eax as *const u32;
                    result = virtual_call(variant, 0x10, result.eax, read_unaligned(variant.cast()), &[]);
                    let count = result.eax;
                    entry.put(1, count);
                    if count != 0 {
                        let first = index.wrapping_add(0x2010);
                        result.eax = first;
                        let mut i = 0u32;
                        loop {
                            let target = read_unaligned(data.wrapping_add(i as usize));
                            let value = read_unaligned(data.wrapping_add(i.wrapping_add(1) as usize));
                            let slot = if target == first {
                                1
                            } else if target == index.wrapping_add(0x2020) {
                                2
                            } else if target == index.wrapping_add(0x2030) {
                                3
                            } else {
                                0
                            };
                            if slot != 0 {
                                let mut factor = F80::from_i32(value as i32);
                                if (value as i32) < 0 {
                                    factor = factor + F80::from_f32(read_unaligned(unsigned_bias()));
                                }
                                factor = factor * F80::from_f32(read_unaligned(percent_scale()));
                                write_unaligned(output.add(slot), factor.to_f32().to_bits());
                            }
                            result.edx = count;
                            i = i.wrapping_add(2);
                            if i >= count {
                                break;
                            }
                        }
                    }
                }
            }
        }
        let resource = entry.arg(0) as *mut u8;
        if !resource.is_null() {
            result = virtual_call(resource, 8, read_unaligned(resource.cast()), result.edx, &[]);
        }
        registers(result.eax, result.edx)
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_006a23a0::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{call_thiscall_with_eax, fake_data, Fake, GameFpu};

    #[derive(Debug, PartialEq)]
    enum Call {
        Lookup([u32; 3], u32, u32, u32),
        Holder,
        Property(u32),
        Variant,
        Data,
        Count,
        Release,
    }

    #[repr(C)]
    struct Resource {
        table: *const usize,
        calls: Vec<Call>,
        data: Vec<u32>,
        fail: u8,
        count: u32,
        output: *mut u32,
    }

    extern "thiscall" fn lookup(
        this: *mut Resource,
        key: *const u32,
        iid: u32,
        output: *mut u32,
        a: u32,
        b: u32,
    ) -> u64 {
        unsafe {
            let r = &mut *this;
            assert_eq!(std::slice::from_raw_parts(r.output.add(1), 3), &[0; 3]);
            assert_eq!(read_unaligned(output), 0);
            r.calls.push(Call::Lookup([*key, *key.add(1), *key.add(2)], iid, a, b));
            if r.fail != 1 {
                write_unaligned(output, this as u32);
            }
            registers(
                if r.fail == 1 || r.fail == 2 {
                    0x1122_3300
                } else {
                    0x1122_3301
                },
                0x1111_1111,
            )
        }
    }
    extern "thiscall" fn holder(this: *mut Resource) -> u64 {
        unsafe {
            (*this).calls.push(Call::Holder);
            registers(if (*this).fail == 3 { 0 } else { this as u32 }, 0x2222_2222)
        }
    }
    extern "thiscall" fn property(this: *mut Resource, id: u32) -> u64 {
        unsafe {
            (*this).calls.push(Call::Property(id));
            registers(if (*this).fail == 4 { 0 } else { this as u32 }, 0x3333_3333)
        }
    }
    extern "thiscall" fn variant(this: *mut Resource) -> u64 {
        unsafe {
            (*this).calls.push(Call::Variant);
        }
        registers(this as u32, 0x4444_4444)
    }
    extern "thiscall" fn data(this: *mut Resource) -> u64 {
        unsafe {
            (*this).calls.push(Call::Data);
            registers((*this).data.as_ptr() as u32, 0x5555_5555)
        }
    }
    extern "thiscall" fn count(this: *mut Resource) -> u64 {
        unsafe {
            (*this).calls.push(Call::Count);
            registers((*this).count, 0x6666_6666)
        }
    }
    extern "thiscall" fn release(this: *mut Resource) -> u64 {
        unsafe {
            (*this).calls.push(Call::Release);
        }
        0x8877_6655_4433_2211
    }

    struct World {
        resource: Box<Resource>,
        _table: Box<[usize; 104]>,
        output: Box<[u32; 5]>,
        _manager: Box<*mut u8>,
        _bias: Box<f32>,
        _scale: Box<f32>,
        _guards: Vec<Fake>,
    }
    impl World {
        fn new(data_values: &[u32], count_value: u32, fail: u8) -> Self {
            let mut table = Box::new([0; 104]);
            for (slot, f) in [
                (8, release as *const ()),
                (0x0C, lookup as *const ()),
                (0x10, count as *const ()),
                (0x14, variant as *const ()),
                (0x1C, holder as *const ()),
                (0x24, property as *const ()),
                (0x19C, data as *const ()),
            ] {
                table[slot / 4] = f as usize;
            }
            let mut output = Box::new([0x7FA1_2345, 7, 8, 9, 0xAABB_CCDD]);
            let mut resource = Box::new(Resource {
                table: table.as_ptr(),
                calls: Vec::new(),
                data: data_values.to_vec(),
                fail,
                count: count_value,
                output: output.as_mut_ptr(),
            });
            let mut manager = Box::new((&mut *resource as *mut Resource).cast());
            let mut bias = Box::new(4_294_967_296.0f32);
            let mut scale = Box::new(0.01f32);
            let guards = vec![
                fake_data(0x00B4_3CA8, (&mut *manager as *mut *mut u8).cast()),
                fake_data(0x00A8_0AA8, (&mut *bias as *mut f32).cast()),
                fake_data(0x00A8_23A4, (&mut *scale as *mut f32).cast()),
            ];
            Self {
                resource,
                _table: table,
                output,
                _manager: manager,
                _bias: bias,
                _scale: scale,
                _guards: guards,
            }
        }
        fn run(&mut self, id: u32, index: u32) -> (u32, u32) {
            unsafe {
                call_thiscall_with_eax(
                    fc4p_006a23a0 as *const () as usize,
                    core::ptr::null_mut(),
                    &[self.output.as_mut_ptr() as u32, id, index],
                    0xCCCC_CCCC,
                )
            }
        }
    }

    #[test]
    fn jobs_factors_keep_unsigned_integer_rounding_and_overwrite_duplicates() {
        let _fpu = GameFpu::new();
        // Cases moved from the core's link-factor test, including both sides of the signed boundary.
        for (value, bits) in [
            (33, 0x3EA8_F5C2),
            (1, 0x3C23_D70A),
            (16_777_217, 0x4823_D70B),
            (3_242_996_796, 0x4BF7_6BC7),
            (u32::MAX, 0x4C23_D70A),
        ] {
            let mut world = World::new(
                &[0x2010, 100, 0x7777, 9, 0x2010, value, 0x2020, 300, 0x2030, 400],
                10,
                0,
            );
            assert_eq!(world.run(0x3111, 0), (0x4433_2211, 0x8877_6655));
            assert_eq!(
                *world.output,
                [0x7FA1_2345, bits, 3.0f32.to_bits(), 4.0f32.to_bits(), 0xAABB_CCDD]
            );
            assert_eq!(
                world.resource.calls,
                vec![
                    Call::Lookup([0x6534_284A, 0xC7BB_4816, 0x3111], 0xA521_60F5, 0, 0),
                    Call::Holder,
                    Call::Property(0x47BB_3F30),
                    Call::Variant,
                    Call::Data,
                    Call::Count,
                    Call::Release
                ]
            );
        }
    }

    #[test]
    fn missing_resource_holder_and_property_clear_outputs_and_release_acquired_resource() {
        for fail in 1..=4 {
            let mut world = World::new(&[], 0, fail);
            let result = world.run(1, 2);
            assert_eq!(world.output[1..4], [0; 3]);
            let mut expected = vec![Call::Lookup([0x6534_284A, 0xC7BB_4816, 3], 0xA521_60F5, 0, 0)];
            if fail >= 3 {
                expected.push(Call::Holder);
            }
            if fail >= 4 {
                expected.push(Call::Property(0x47BB_3F30));
            }
            if fail != 1 {
                expected.push(Call::Release);
            }
            assert_eq!(world.resource.calls, expected);
            assert_eq!(
                result,
                if fail == 1 {
                    (0x1122_3300, 0x1111_1111)
                } else {
                    (0x4433_2211, 0x8877_6655)
                }
            );
        }
    }

    #[test]
    fn empty_and_odd_element_counts_and_wrapping_indices_follow_the_lookup() {
        let _fpu = GameFpu::new();
        let mut empty = World::new(&[], 0, 0);
        empty.run(7, u32::MAX);
        assert_eq!(empty.output[1..4], [0; 3]);
        let mut odd = World::new(&[0x200F, 33], 1, 0);
        odd.run(7, u32::MAX);
        assert_eq!(odd.output[1..4], [0x3EA8_F5C2, 0, 0]);
        assert_eq!(
            odd.resource.calls[0],
            Call::Lookup([0x6534_284A, 0xC7BB_4816, 6], 0xA521_60F5, 0, 0)
        );
    }

    #[test]
    fn lookup_reuses_argument_words_and_preserves_existing_x87_values_and_control_state() {
        use crate::demand::test_support::Probe;
        let _fpu = GameFpu::new();
        let mut world = World::new(&[0x2010, 33], 2, 0);
        let mut probe = Probe::new(
            fc4p_006a23a0 as *const (),
            core::ptr::null_mut(),
            [world.output.as_mut_ptr() as u32, 0x3111, 0],
        );
        #[repr(C, align(16))]
        struct Fpu([u8; 512]);
        let mut before = Fpu([0; 512]);
        let mut after = Fpu([0; 512]);
        unsafe {
            core::arch::asm!("fld1", "fxsave [{p}]", p = in(reg) &mut before, out("st(0)") _);
            probe.run();
            core::arch::asm!("fxsave [{p}]", "fstp st(0)", p = in(reg) &mut after, out("st(0)") _);
        }
        assert_eq!(probe.after, [&mut *world.resource as *mut Resource as u32, 2, 0]);
        // Status flags and last-instruction pointers can change during arithmetic. The stack contents,
        // stack position, control word and MXCSR must retain their entry values.
        assert_eq!(before.0[0..2], after.0[0..2]);
        assert_eq!(before.0[3] & 0x38, after.0[3] & 0x38);
        assert_eq!(before.0[4], after.0[4]);
        assert_eq!(before.0[24..28], after.0[24..28]);
        assert_eq!(before.0[32..42], after.0[32..42]);
    }
}
