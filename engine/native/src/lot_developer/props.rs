// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Prop creation from explicit instances or randomly selected configuration families.

use super::*;

game_data! { static position_initial: u32 = 0x00B4_9AEC; }
game_data! { static prop_resources: *mut u8 = 0x00B4_3D18; }
game_data! { static random_state: u8 = 0x00B4_9AF0; }
game_fn! { fn random_index(this: *mut u8, count: u32) -> u64 = "thiscall" 0x0091_36B1; }
game_fn! { #[allow(dead_code)] fn free_instances(storage: *mut u8) -> u64 = "C" 0x0090_CF63; }
// EDX is an additional register input; the two ordinary arguments still occupy eight callee-popped bytes.
game_fn! { fn create_family_prop(this: *mut u8, edx: u32, lot: *mut u8, index: i32) -> u64 = "fastcall" 0x006C_6D70; }

layout! {
    /// Fields used from one 52-byte configuration object.
    pub struct ConfigurationObject {
        0x00 => kind / set_kind: u8,
        0x01 => flags / set_flags: u8,
        0x24 => appearance / set_appearance: u32,
    }
}

port! {
    /// Positions the indexed configuration object, creates the requested nonzero instance and applies
    /// its appearance word. The position starts with three copies of the game's initial-position word;
    /// facing starts with the lot pointer's bits and is passed to the positioning callback by reference.
    #[port(address = 0x006C_6230, owner = "lotdeveloper", name = "cSC4LotDeveloper::CreateLotProp", naked)]
    pub unsafe extern "fastcall" fn fc4p_006c6230(this: *mut u8, entry_edx: u32, lot: *mut u8, index: i32, instance: u32) -> u64 {
        stack_entry!(create_lot_prop, 12);
    }
}

unsafe fn prop_call(object: *mut u8, slot: usize, args: &[u32], eax: u32, edx: u32) -> RawCall {
    unsafe {
        let mut call = RawCall::new(Conv::Thiscall, vslot(object, slot), object as u32, args);
        call.eax = eax;
        call.edx = edx;
        call_with_ecx(&mut call);
        call
    }
}

// Facing overwrites the lot argument; position occupies the three words below the return address.
unsafe extern "fastcall" fn create_lot_prop(this: *mut u8, stack: *mut u32, entry_edx: u32) -> u64 {
    unsafe {
        let lot = stack.add(1).read() as *mut u8;
        let index = stack.add(2).read() as i32;
        if lot.is_null() { return u64::from(entry_edx) << 32; }
        let mut call = prop_call(lot, 0x24, &[], table(lot), entry_edx);
        let config = call.eax as *mut u8;
        if config.is_null() { stack.sub(1).write(call.ecx); return result(call); }
        call = prop_call(config, 0x5C, &[], call.eax, table(config));
        let begin = (call.eax as *const *mut u8).read_unaligned();
        let record = begin.wrapping_add(index.wrapping_mul(52) as usize);
        let initial = position_initial().read();
        let position = stack.sub(3);
        position.cast::<[u32; 3]>().write([initial; 3]);
        let facing = stack.add(1);
        call = prop_call(this, 0x9C,
            &[lot as u32, config as u32, record as u32, position as u32, facing as u32],
            table(this), position as u32);
        let instance = stack.add(3).read();
        if instance != 0 {
            let manager = LotDeveloper::from_ptr(this).prop_manager();
            call = prop_call(manager, 0x68, &[instance, position as u32, facing.read()], instance, table(manager));
            let prop = call.eax as *mut u8;
            if !prop.is_null() {
                let appearance = ConfigurationObject::from_ptr(record).appearance();
                call = prop_call(prop, 0x70, &[appearance], appearance, table(prop));
                call.eax = prop as u32;
                stack.sub(1).write(call.ecx);
                return result(call);
            }
        }
        stack.sub(1).write(call.ecx);
        call.eax = 0;
        result(call)
    }
}

port! {
    /// Uses the prop resource service to enumerate instances for the indexed object, chooses one through
    /// the game's random generator, positions and creates it, and applies the object's appearance.
    /// The temporary instance array is freed on both success and failure without changing its elements.
    #[port(address = 0x006C_6D70, owner = "lotdeveloper", name = "CreateFamilyLotProp", naked)]
    pub unsafe extern "C" fn fc4p_006c6d70() {
        // The private result buffer also carries entry EDX. The body computes in Rust; this entry
        // supplies the additional ECX result channel that its caller can observe.
        core::arch::naked_asm!(
            "sub esp, 12", "mov [esp + 4], edx", "mov edx, esp",
            "push dword ptr [esp + 20]", "push dword ptr [esp + 20]", "call {}",
            "mov eax, [esp]", "mov edx, [esp + 4]", "mov ecx, [esp + 8]", "add esp, 12", "ret 8",
            sym family_prop_entry,
        );
    }
}

unsafe extern "fastcall" fn family_prop_entry(this: *mut u8, registers: *mut [u32; 3], lot: *mut u8, index: i32) {
    unsafe {
        let entry_edx = registers.cast::<u32>().add(1).read();
        registers.write(family_prop_registers(this, entry_edx, lot, index));
    }
}

/// Calls with the ordinary RawCall inputs while also retaining ECX before any caller cleanup.
unsafe fn call_with_ecx(call: &mut RawCall) {
    unsafe {
        core::arch::asm!(
            "mov ecx, [edi + 20]", "mov eax, [edi + 16]",
            "2:", "test ecx, ecx", "jz 3f", "dec ecx", "push dword ptr [eax + ecx*4]", "jmp 2b",
            "3:", "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]", "call dword ptr [edi]",
            "mov [edi + 4], eax", "mov [edi + 8], ecx", "mov [edi + 12], edx",
            "cmp dword ptr [edi + 24], 0", "je 4f", "mov ecx, [edi + 20]", "lea esp, [esp + ecx*4]", "4:",
            in("edi") call as *mut RawCall, clobber_abi("C"),
        );
    }
}

unsafe fn family_prop_registers(this: *mut u8, entry_edx: u32, lot: *mut u8, index: i32) -> [u32; 3] {
    unsafe {
        if lot.is_null() {
            return [0, entry_edx, this as u32];
        }
        let mut call = RawCall::new(Conv::Thiscall, vslot(lot, 0x24), lot as u32, &[]);
        call.eax = table(lot);
        call.edx = entry_edx;
        call_with_ecx(&mut call);
        let config = call.eax as *mut u8;
        if config.is_null() {
            return [call.eax, call.edx, call.ecx];
        }
        call = virtual_call(config, 0x5C, &[], call.eax, table(config), false);
        let begin = (call.eax as *const *mut u8).read_unaligned();
        let record = begin.wrapping_add(index.wrapping_mul(52) as usize);
        let initial = position_initial().read();
        let mut position = [initial; 3];
        let mut facing = lot as u32;
        virtual_call(
            this,
            0x9C,
            &[
                lot as u32,
                config as u32,
                record as u32,
                position.as_mut_ptr() as u32,
                &mut facing as *mut u32 as u32,
            ],
            table(this),
            position.as_mut_ptr() as u32,
            false,
        );
        let mut instances = [0u32; 3];
        let resources = prop_resources().read();
        call = virtual_call(
            resources,
            0x6C,
            &[record as u32, instances.as_mut_ptr() as u32],
            table(resources),
            instances.as_mut_ptr() as u32,
            false,
        );
        let mut prop = core::ptr::null_mut();
        call.ecx = instances[0];
        call.eax = instances[1];
        if instances[0] != instances[1] {
            let count = (instances[1].wrapping_sub(instances[0]) as i32 >> 2) as u32;
            let returned = random_index(random_state(), count);
            call.edx = (returned >> 32) as u32;
            let instance = (instances[0] as *const u32)
                .wrapping_add(returned as u32 as usize)
                .read_unaligned();
            call.ecx = instances[0];
            call.eax = instance;
            if instance != 0 {
                let manager = LotDeveloper::from_ptr(this).prop_manager();
                call = virtual_call(
                    manager,
                    0x68,
                    &[instance, position.as_mut_ptr() as u32, facing],
                    instance,
                    table(manager),
                    false,
                );
                prop = call.eax as *mut u8;
                call.ecx = instances[0];
                if !prop.is_null() {
                    let appearance = ConfigurationObject::from_ptr(record).appearance();
                    let args = [appearance];
                    call = RawCall::new(Conv::Thiscall, vslot(prop, 0x70), prop as u32, &args);
                    call.eax = appearance;
                    call.edx = table(prop);
                    call_with_ecx(&mut call);
                    call.eax = instances[0];
                }
            }
        }
        if instances[0] != 0 {
            let args = [instances[0]];
            let mut free_call = RawCall::new(Conv::Cdecl, crate::abi::target(0x0090_CF63), call.ecx, &args);
            free_call.eax = call.eax;
            free_call.edx = call.edx;
            call_with_ecx(&mut free_call);
            call = free_call;
        }
        [prop as u32, call.edx, call.ecx]
    }
}

port! {
    /// Walks the configuration's object array with its initial count, creating family props for kind one
    /// when flag bit zero is clear. The array's begin pointer is reread after every creation callback.
    /// The third stack argument is accepted and ignored.
    #[port(address = 0x006C_7860, owner = "lotdeveloper", name = "CreateUnflaggedFamilyProps")]
    pub unsafe extern "fastcall" fn fc4p_006c7860(this: *mut u8, entry_edx: u32, lot: *mut u8, config: *mut u8, _flags: u32) -> u64 {
        unsafe {
            let mut call = virtual_call(config, 0x5C, &[], table(config), entry_edx, false);
            let vector = call.eax as *const u32;
            if vector.is_null() { return result(call); }
            let difference = vector.add(1).read_unaligned().wrapping_sub(vector.read_unaligned()) as i32;
            let product = i64::from(difference) * 0x4EC4_EC4F;
            let high = (product >> 32) as i32 >> 4;
            let count = high.wrapping_add((high as u32 >> 31) as i32);
            call.eax = product as u32;
            call.edx = high as u32;
            let mut index = 0i32;
            let mut offset = 0u32;
            while index < count {
                let record = (vector.read_unaligned() as *mut u8).wrapping_add(offset as usize);
                call.eax = record as u32;
                let object = ConfigurationObject::from_ptr(record);
                if object.kind() == 1 && object.flags() & 1 == 0 {
                    let returned = create_family_prop(this, call.edx, lot, index);
                    call.eax = returned as u32;
                    call.edx = (returned >> 32) as u32;
                }
                index = index.wrapping_add(1);
                offset = offset.wrapping_add(52);
            }
            result(call)
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_006c6230::PORT, fc4p_006c6d70::PORT, fc4p_006c7860::PORT];
