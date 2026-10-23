// SPDX-License-Identifier: GPL-3.0-or-later
//! Portable cGZPersistResourceManager, exclusively from persistence-manager.md
//! §§1–11. Owns registry, dispatch, failure and lifetime algorithms; the host
//! supplies only foreign resource/factory/record/segment/filter/cache primitives.
//! Symbolic handles are not native vtables or ABI pointer layouts.
mod cache;
mod manager;
mod primitives;
mod table;
#[cfg(test)]
mod test_host;

pub use cache::{BuiltinCache, CacheId, CacheStore};
pub use manager::{InterfaceView, PersistManager, SegmentVisitor, ServiceState};
pub use primitives::{Lock, Object, Primitives};

pub const RESOURCE_IID: u32 = 0x456b8f1d;
pub const FACTORY_IID: u32 = 0xa56b8f17;
pub const MANAGER_IID: u32 = 0x656b8efc;
pub const SERVICE_IID: u32 = 0x287fb697;
pub const CACHE_IID: u32 = 0x6bb923e4;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Key {
    pub ty: u32,
    pub group: u32,
    pub instance: u32,
}

impl Key {
    pub const fn new(ty: u32, group: u32, instance: u32) -> Self {
        Self { ty, group, instance }
    }
}
impl table::HashKey for Key {
    fn hash(self) -> u32 {
        self.instance
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    Type(u32),
    Foreign(Object),
}
impl Filter {
    pub fn accepts(self, host: &mut impl Primitives, key: Key) -> bool {
        match self {
            Self::Type(ty) => key.ty == ty,
            Self::Foreign(object) => host.filter_accepts(object, key),
        }
    }
    fn add_ref(self, host: &mut impl Primitives) {
        if let Self::Foreign(object) = self {
            host.add_ref(object);
        }
    }
    fn release(self, host: &mut impl Primitives) {
        if let Self::Foreign(object) = self {
            host.release(object);
        }
    }
}

/// Referenced result list, persistence §5: insertion order, duplicates allowed.
/// The Rust value owns the allocation. Its logical COM reference starts at one.
#[derive(Debug)]
pub struct KeyList {
    keys: Vec<Key>,
    references: u32,
}
impl Default for KeyList {
    fn default() -> Self {
        Self::new()
    }
}
impl KeyList {
    pub fn new() -> Self {
        Self {
            keys: Vec::new(),
            references: 1,
        }
    }
    pub fn insert(&mut self, key: Key) -> bool {
        self.keys.push(key);
        true
    }
    /// Section 5 specifies append effects, but no Insert(list) return guarantee.
    /// Distinct borrowed owners only; native self-insertion remains unverified.
    pub fn insert_list(&mut self, source: &Self) {
        source.enum_keys(|key| {
            self.insert(key);
        });
    }
    pub fn erase(&mut self, key: Key) -> bool {
        if let Some(index) = self.keys.iter().position(|candidate| *candidate == key) {
            self.keys.remove(index);
            true
        } else {
            false
        }
    }
    pub fn erase_all(&mut self) -> bool {
        self.keys.clear();
        true
    }
    pub fn is_present(&self, key: Key) -> bool {
        self.keys.contains(&key)
    }
    /// Valid indices only. Native invalid-index behaviour is not emulated.
    pub fn get_key(&self, index: u32) -> Key {
        self.keys[index as usize]
    }
    /// Copies, in order; no mutating/reentrant callback contract is assumed.
    pub fn enum_keys(&self, mut callback: impl FnMut(Key)) {
        for key in &self.keys {
            callback(*key);
        }
    }
    pub fn keys(&self) -> &[Key] {
        &self.keys
    }
    pub fn size(&self) -> u32 {
        self.keys.len() as u32
    }
    pub fn reference_count(&self) -> u32 {
        self.references
    }
    pub fn add_ref(&mut self) -> u32 {
        self.references = self.references.wrapping_add(1);
        self.references
    }
    pub fn release(&mut self) -> u32 {
        if self.references > 1 {
            self.references -= 1;
        } else {
            self.references = 0;
            self.keys.clear();
        }
        self.references
    }
}
