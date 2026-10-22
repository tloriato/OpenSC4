// SPDX-License-Identifier: GPL-3.0-or-later
//! Private, bounded native owner transport. Not a replacement registration.
//! Foreign VM/generator operations are callbacks, never ScriptServer oracles.
use fc4_core::lua_script_server::{Failure, Foreign, Handle, Primitive, ScriptServer, Value};
use std::sync::Mutex;

/// Lifecycle callback opcodes. Pointer-sized handles are borrowed except where
/// the primitive's documented factory/acquisition contract transfers a reference.
#[repr(C)]
pub struct Call {
    pub op: u32,
    pub handle: u64,
    pub other: u64,
    pub number: u32,
}
type Callback = unsafe extern "C" fn(*mut std::ffi::c_void, *const Call, *mut u64) -> bool;

pub struct Owner {
    server: Mutex<ScriptServer>,
}
struct Host {
    user: *mut std::ffi::c_void,
    callback: Callback,
}
impl Foreign for Host {
    fn invoke(&mut self, _: &mut ScriptServer, primitive: Primitive) -> Result<Value, Failure> {
        let mut call = Call {
            op: 0,
            handle: 0,
            other: 0,
            number: 0,
        };
        let kind = match primitive {
            Primitive::CreateLua { class, iid } => {
                call.op = 1;
                call.number = class;
                call.other = iid.into();
                1
            }
            Primitive::Open { lua, libraries } => {
                call.op = 2;
                call.handle = lua.0;
                call.number = libraries;
                2
            }
            Primitive::Close(h) => {
                call.op = 3;
                call.handle = h.0;
                0
            }
            Primitive::CreateGenerator { class, iid } => {
                call.op = 4;
                call.number = class;
                call.other = iid.into();
                1
            }
            Primitive::InitGenerator {
                generator,
                lua,
                global: "sc4app",
            } => {
                call.op = 5;
                call.handle = generator.0;
                call.other = lua.0;
                2
            }
            Primitive::GeneratorTarget(h) => {
                call.op = 6;
                call.handle = h.0;
                1
            }
            Primitive::ShutdownGeneratorTarget(h) => {
                call.op = 7;
                call.handle = h.0;
                0
            }
            Primitive::FrameworkAvailable => {
                call.op = 8;
                2
            }
            Primitive::RegisterService => {
                call.op = 9;
                0
            }
            Primitive::UnregisterService => {
                call.op = 10;
                0
            }
            Primitive::Release { handle, .. } => {
                call.op = 11;
                call.handle = handle.0;
                0
            }
            Primitive::BaseDestroy => {
                call.op = 12;
                0
            }
            Primitive::FreeAllocation => {
                call.op = 13;
                0
            }
            // The bounded bridge refuses before invoking any unbound ABI. In
            // particular no dummy arg closure, thread execution or debug layout
            // is supplied as though it were a production implementation.
            _ => return Err(Failure::Foreign("native ScriptServer primitive not bound")),
        };
        let mut reply = 0;
        if !unsafe { (self.callback)(self.user, &call, &mut reply) } {
            return Err(Failure::Foreign("native ScriptServer callback refused"));
        }
        Ok(match kind {
            1 => Value::Handle((reply != 0).then_some(Handle(reply))),
            2 => Value::Bool(reply != 0),
            _ => Value::Unit,
        })
    }
}

#[no_mangle]
pub extern "C" fn fc4_luascripts_new() -> *mut Owner {
    Box::into_raw(Box::new(Owner {
        server: Mutex::new(ScriptServer::new()),
    }))
}

/// Caller has exclusive allocation ownership and no active callbacks/references.
#[no_mangle]
pub unsafe extern "C" fn fc4_luascripts_free(owner: *mut Owner) {
    if !owner.is_null() {
        drop(unsafe { Box::from_raw(owner) });
    }
}

/// Returns transport success separately from the owner's result. Busy/reentrant
/// entry is refused rather than constructing overlapping mutable Rust references.
/// op: 1 AddRef, 2 Release, 3 Init, 4 Shutdown, 5 plain destruction;
/// 6 current, 7 last, 8 refcount, 9 auto-yield get, 10 auto-yield set, 11 borrowed Lua.
#[no_mangle]
pub unsafe extern "C" fn fc4_luascripts_call(
    owner: *mut Owner,
    callback: Option<Callback>,
    user: *mut std::ffi::c_void,
    op: u32,
    id: i32,
    flag: u32,
    output: *mut i64,
) -> bool {
    let (Some(owner), Some(output), Some(callback)) = (unsafe { owner.as_ref() }, unsafe { output.as_mut() }, callback)
    else {
        return false;
    };
    let Ok(mut server) = owner.server.try_lock() else {
        return false;
    };
    let mut host = Host { user, callback };
    let result = match op {
        1 => server.add_ref().map(i64::from),
        2 => server.release(&mut host).map(i64::from),
        3 => server.init(&mut host).map(i64::from),
        4 => server.shutdown(&mut host).map(i64::from),
        5 => server.deleting_destructor(&mut host, flag).map(|()| 0),
        6 => server.get_current_running_script_id(&mut host).map(i64::from),
        7 => Ok(i64::from(server.get_last_created_script_id())),
        8 => Ok(i64::from(server.ref_count())),
        9 => Ok(i64::from(server.get_script_auto_yield_enabled(id))),
        10 => Ok(i64::from(server.set_script_auto_yield_enabled(id, flag != 0))),
        11 => Ok(server.get_lua().map_or(0, |handle| handle.0 as i64)),
        _ => return false,
    };
    match result {
        Ok(value) => {
            *output = value;
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    unsafe extern "C" fn absent(_: *mut std::ffi::c_void, call: *const Call, reply: *mut u64) -> bool {
        unsafe {
            *reply = 0;
            matches!((*call).op, 1 | 8 | 10 | 12 | 13)
        }
    }
    #[test]
    fn failed_init_latch_release_zero_and_null_transport() {
        unsafe {
            let owner = fc4_luascripts_new();
            let mut result = -77;
            let mut call = |op| fc4_luascripts_call(owner, Some(absent), std::ptr::null_mut(), op, -1, 0, &mut result);
            assert!(call(3)); // false owner result, not transport failure
            assert!(call(3)); // no retry: latch is retained
            assert!(call(2)); // Release at zero shuts down and destroys
            assert!(!call(1)); // destroyed owner refuses another reference
            assert!(!fc4_luascripts_call(
                std::ptr::null_mut(),
                Some(absent),
                std::ptr::null_mut(),
                1,
                0,
                0,
                &mut result
            ));
            fc4_luascripts_free(owner);
        }
    }

    #[test]
    fn refused_unbound_stack_retains_cleanup_and_borrowed_target_order() {
        unsafe extern "C" fn foreign(user: *mut std::ffi::c_void, call: *const Call, reply: *mut u64) -> bool {
            let calls = unsafe { &mut *user.cast::<Vec<u32>>() };
            let call = unsafe { &*call };
            calls.push(call.op);
            unsafe {
                *reply = match call.op {
                    1 => 0x101, // newly acquired Lua
                    4 => 0x202, // newly acquired generator
                    6 => 0x303, // borrowed sibling: must not be released
                    2 | 5 | 8 => 1,
                    _ => 0,
                };
            }
            if call.op == 11 {
                assert!(matches!(call.handle, 0x101 | 0x202));
            }
            true
        }
        unsafe {
            let owner = fc4_luascripts_new();
            let mut calls = Vec::<u32>::new();
            let user = (&mut calls as *mut Vec<u32>).cast();
            let mut result = -77;
            // Init reaches the first unbound primitive (MainThread); it cannot
            // masquerade as a successful owner result or erase the failure.
            assert!(!fc4_luascripts_call(owner, Some(foreign), user, 3, -1, 0, &mut result));
            assert_eq!(result, -77);
            assert_eq!(calls, [1, 2, 4, 5, 8, 9]);
            calls.clear();
            assert!(fc4_luascripts_call(owner, Some(foreign), user, 4, -1, 0, &mut result));
            assert_eq!(result, 1);
            assert_eq!(calls, [8, 10, 6, 7, 11, 3, 11]);
            calls.clear();
            assert!(fc4_luascripts_call(owner, Some(foreign), user, 2, -1, 0, &mut result));
            assert_eq!(calls, [12, 13]);
            fc4_luascripts_free(owner);
        }
    }

    #[test]
    fn busy_owner_refuses_without_touching_output() {
        unsafe {
            let owner = fc4_luascripts_new();
            let lock = (*owner).server.lock().unwrap();
            let mut result = -77;
            assert!(!fc4_luascripts_call(
                owner,
                Some(absent),
                std::ptr::null_mut(),
                8,
                -1,
                0,
                &mut result
            ));
            assert_eq!(result, -77);
            drop(lock);
            fc4_luascripts_free(owner);
        }
    }
}
