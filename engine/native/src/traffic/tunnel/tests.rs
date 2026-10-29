// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, GameFpu, Object};
use crate::traffic::test_support::{bytes, table};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Position(u32),
    Cell(u32, u32),
    Pair(u32),
    AddRef(u32),
    OtherPosition(u32),
    Release(u32),
    Type(u32),
    Erase(u32, u32),
    Entry(u32, u32),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    paired: u32,
    other_position: u32,
    positions: [[f32; 3]; 2],
    cells: [[i32; 2]; 2],
    cell_index: usize,
    network: Option<u32>,
    record: u32,
}
thread_local! {static STATE:RefCell<State>=RefCell::new(State::default());}
fn take() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}
extern "thiscall" fn position_fake(this: *mut u8, out: *mut [f32; 3]) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Position(this as u32));
        let i = s.cell_index;
        unsafe {
            out.write_unaligned(s.positions[i]);
        }
        0
    })
}
extern "thiscall" fn cell_fake(_this: *mut u8, x: u32, z: u32, out_x: *mut i32, out_z: *mut i32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Cell(x, z));
        let i = s.cell_index;
        unsafe {
            out_x.write_unaligned(s.cells[i][0]);
            out_z.write_unaligned(s.cells[i][1]);
        }
        s.cell_index += 1;
        0
    })
}
extern "thiscall" fn pair_fake(this: *mut u8) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Pair(this as u32));
        s.paired
    })
}
extern "thiscall" fn add_fake(this: *mut u8) -> u32 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::AddRef(this as u32)));
    1
}
extern "thiscall" fn release_fake(this: *mut u8) -> u32 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Release(this as u32)));
    0
}
extern "thiscall" fn other_position_fake(this: *mut u8) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::OtherPosition(this as u32));
        s.other_position
    })
}
extern "thiscall" fn type_fake(_this: *mut u8, network: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Type(network));
        0xAABB_CC00 | u32::from(s.network == Some(network))
    })
}
extern "thiscall" fn erase_fake(this: *mut u8, key: *const u32) -> u32 {
    STATE.with(|s| {
        s.borrow_mut()
            .calls
            .push(Call::Erase(this as u32, unsafe { key.read_unaligned() }))
    });
    0
}
extern "thiscall" fn entry_fake(this: *mut u8, key: *const u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Entry(this as u32, unsafe { key.read_unaligned() }));
        s.record
    })
}

/// Seeds the exact two bytes below the entry return address that the record retains as padding.
unsafe fn invoke(this: *mut u8, occupant: *mut u8, inserted: u32) -> Registers {
    let mut frame = [
        fc4p_007140e0 as *const () as u32,
        this as u32,
        occupant as u32,
        inserted,
        0,
        0,
        0,
    ];
    unsafe {
        core::arch::asm!("push dword ptr [edi + 12]","push dword ptr [edi + 8]","mov ecx, [edi + 4]",
        "mov word ptr [esp - 6], 0xBEEF","call dword ptr [edi]","mov [edi + 16], eax","mov [edi + 20], ecx","mov [edi + 24], edx",in("edi") frame.as_mut_ptr(),clobber_abi("C"));
    }
    Registers {
        eax: frame[4],
        ecx: frame[5],
        edx: frame[6],
    }
}

/// The core's tunnel-cost and type-order vectors, including removal, missing pairs, unsupported
/// networks, byte coordinates, exact x87 operation order and retained record padding.
#[test]
fn portal_changes_preserve_call_order_network_priority_cost_bits_and_stack_padding() {
    unsafe {
        let _pair = fake(0x0064_7570, pair_fake as *const () as usize);
        let _erase = fake(0x0071_0910, erase_fake as *const () as usize);
        let _entry = fake(0x0071_3D60, entry_fake as *const () as usize);
        let mut masks = [0u32; 13];
        let _masks = fake_data(0x00AB_3174, masks.as_mut_ptr().cast());
        let mut speeds = [[f32::MAX; 13]; 9];
        for (travel, row) in speeds.iter_mut().enumerate() {
            for (network, value) in row.iter_mut().enumerate() {
                *value = (travel * 13 + network + 1) as f32 * 0.125;
            }
        }
        speeds[0][0] = f32::MAX;
        speeds[1][0] = f32::from_bits(0x3CFD_B011);
        speeds[2][0] = f32::from_bits(0x3E8C_6F2E);
        let _speeds = fake_data(0x00B4_B110, speeds.as_mut_ptr().cast());
        let mut scale = 0.0625f32;
        let _scale = fake_data(0x00A8_E174, (&mut scale as *mut f32).cast());
        let _fpu = GameFpu::new();
        let ov = table(&[(0x58, type_fake as *const () as usize)]);
        let pv = table(&[(0x20, position_fake as *const () as usize)]);
        let paired_v = table(&[
            (4, add_fake as *const () as usize),
            (8, release_fake as *const () as usize),
            (0xC0, other_position_fake as *const () as usize),
        ]);
        let cv = table(&[(0x17C, cell_fake as *const () as usize)]);
        let mut occupant = [ov.as_ptr() as u32, pv.as_ptr() as u32];
        let op = occupant.as_mut_ptr().cast::<u8>();
        let mut paired = [paired_v.as_ptr() as u32];
        let pp = paired.as_mut_ptr().cast::<u8>();
        let mut position = [pv.as_ptr() as u32];
        let pos = position.as_mut_ptr().cast::<u8>();
        let mut city = [cv.as_ptr() as u32];
        let mut sim = Object::new(0xD60);
        let p = sim.ptr();
        store(p, 0x14, city.as_mut_ptr() as u32);
        for network in [Some(2u32), Some(6), Some(0), Some(1), Some(10), Some(12), Some(9), None] {
            for coordinates in [
                [[8.0f32, 0.0, 24.0], [72.0, 0.0, 24.0]],
                [[1.25, -4.5, 2.0], [5.0, 11.0, -9.0]],
            ] {
                let mut record = [0xA5u8; 48];
                let rp = record.as_mut_ptr();
                let before = bytes(p, 0xD60);
                STATE.with(|s| {
                    *s.borrow_mut() = State {
                        paired: pp as u32,
                        other_position: pos as u32,
                        positions: coordinates,
                        cells: [[0x123, 0x1FE], [0x134, 0x156]],
                        network,
                        record: rp as u32,
                        ..State::default()
                    }
                });
                let r = invoke(p, op, 0xABCD_0001);
                let mut expected = vec![
                    Call::Position(op.add(4) as u32),
                    Call::Cell(coordinates[0][0].to_bits(), coordinates[0][2].to_bits()),
                    Call::Pair(op as u32),
                    Call::AddRef(pp as u32),
                    Call::OtherPosition(pp as u32),
                    Call::Position(pos as u32),
                    Call::Cell(coordinates[1][0].to_bits(), coordinates[1][2].to_bits()),
                    Call::Release(pp as u32),
                ];
                for ty in [2u32, 6, 0, 1, 10, 12, 9] {
                    expected.push(Call::Type(ty));
                    if network == Some(ty) {
                        break;
                    }
                }
                if let Some(network) = network {
                    expected.push(Call::Entry(p.add(0xC8) as u32, 0x23FE));
                    assert_eq!(r.eax, rp as u32);
                    assert_eq!(r.ecx, 0);
                    assert_eq!(&record[..4], &[0x34, 0x56, 0xFE, 1]);
                    assert_eq!(&record[40..44], &[network as u8, 0, 0xEF, 0xBE]);
                    assert_eq!(&record[44..], &[0xA5; 4]);
                    let dx = subtract_float(F80::from_f32(coordinates[0][0]), coordinates[1][0]);
                    let dy = subtract_float(F80::from_f32(coordinates[0][1]), coordinates[1][1]);
                    let dz = subtract_float(F80::from_f32(coordinates[0][2]), coordinates[1][2]);
                    let d = ((dz * dz + dy * dy) + dx * dx).sqrt();
                    for (travel, row) in speeds.iter().enumerate() {
                        assert_eq!(
                            word(rp, 4 + travel * 4),
                            multiply_float(multiply_float(d, row[network as usize]), scale)
                                .to_f32()
                                .to_bits()
                        );
                    }
                    if network == 0 && coordinates[0][0] == 8.0 {
                        assert_eq!(word(rp, 8), 0x3DFD_B011);
                        assert_eq!(word(rp, 12), 0x3F8C_6F2E);
                        assert_eq!(word(rp, 4), f32::INFINITY.to_bits());
                    }
                } else {
                    assert_eq!(record, [0xA5; 48]);
                }
                assert_eq!(take(), expected);
                assert_eq!(bytes(p, 0xD60), before);
            }
        }
        STATE.with(|s| {
            *s.borrow_mut() = State {
                positions: [[8.0, 0.0, 24.0]; 2],
                cells: [[0x123, -2], [0, 0]],
                ..State::default()
            }
        });
        invoke(p, op, 0x1234_0000);
        assert_eq!(
            take(),
            [
                Call::Position(op.add(4) as u32),
                Call::Cell(8.0f32.to_bits(), 24.0f32.to_bits()),
                Call::Erase(p.add(0xC8) as u32, 0xFFFF_FFFE)
            ]
        );
        STATE.with(|s| s.borrow_mut().cell_index = 0);
        invoke(p, op, 1);
        assert_eq!(
            take(),
            [
                Call::Position(op.add(4) as u32),
                Call::Cell(8.0f32.to_bits(), 24.0f32.to_bits()),
                Call::Pair(op as u32)
            ]
        );
    }
}
