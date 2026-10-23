// SPDX-License-Identifier: GPL-3.0-or-later
//! Private static-link transport: retained production KeyList owners only.
use fc4_core::persistence_manager::{Key, KeyList};
use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct KeyWords {
    pub ty: u32,
    pub group: u32,
    pub instance: u32,
}
impl From<KeyWords> for Key {
    fn from(k: KeyWords) -> Self {
        Self::new(k.ty, k.group, k.instance)
    }
}
impl From<Key> for KeyWords {
    fn from(k: Key) -> Self {
        Self {
            ty: k.ty,
            group: k.group,
            instance: k.instance,
        }
    }
}
pub struct Session([KeyList; 2]);

#[no_mangle]
pub extern "C" fn fc4_kl_create() -> *mut Session {
    catch_unwind(|| Box::into_raw(Box::new(Session([KeyList::new(), KeyList::new()])))).unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn fc4_kl_destroy(session: *mut Session) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !session.is_null() {
            drop(Box::from_raw(session));
        }
    }));
}

// 0 insert, 1 erase, 2 clear, 3 present, 4 size, 5 append other owner.
// Append has no scored return channel; transport success is separate.
#[no_mangle]
pub unsafe extern "C" fn fc4_kl_op(session: *mut Session, list: u32, op: u32, key: KeyWords, out: *mut u32) -> u32 {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(session) = session.as_mut() else {
            return 0;
        };
        let Some(out) = out.as_mut() else { return 0 };
        if list > 1 {
            return 0;
        }
        let lists = &mut session.0;
        *out = match op {
            0 => u32::from(lists[list as usize].insert(key.into())),
            1 => u32::from(lists[list as usize].erase(key.into())),
            2 => u32::from(lists[list as usize].erase_all()),
            3 => u32::from(lists[list as usize].is_present(key.into())),
            4 => lists[list as usize].size(),
            5 => {
                let (left, right) = lists.split_at_mut(1);
                if list == 0 {
                    left[0].insert_list(&right[0]);
                } else {
                    right[0].insert_list(&left[0]);
                }
                0
            }
            _ => return 0,
        };
        1
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "C" fn fc4_kl_get(session: *const Session, list: u32, index: u32, out: *mut KeyWords) -> u32 {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(session) = session.as_ref() else {
            return 0;
        };
        let Some(list) = session.0.get(list as usize) else {
            return 0;
        };
        let Some(out) = out.as_mut() else { return 0 };
        if index >= list.size() {
            return 0;
        }
        *out = list.get_key(index).into();
        1
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "C" fn fc4_kl_enum(
    session: *const Session,
    list: u32,
    callback: Option<unsafe extern "C" fn(*const KeyWords, *mut c_void)>,
    context: *mut c_void,
) -> u32 {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(session) = session.as_ref() else {
            return 0;
        };
        let Some(list) = session.0.get(list as usize) else {
            return 0;
        };
        let Some(callback) = callback else { return 0 };
        list.enum_keys(|key| callback(&KeyWords::from(key), context));
        1
    }))
    .unwrap_or(0)
}
