// SPDX-License-Identifier: GPL-3.0-or-later
//! Independently authored literal persistence.md section 5 vectors.
use fc4_core::persistence_manager::{Key, KeyList};

const A: Key = Key::new(9, 8, 7);
const T: Key = Key::new(0, 8, 7);
const G: Key = Key::new(9, u32::MAX, 7);
const I: Key = Key::new(9, 8, 0);
const Z: Key = Key::new(0, 0, 0);
const F: Key = Key::new(u32::MAX, u32::MAX, u32::MAX);

fn sequence(list: &KeyList, expected: &[Key]) {
    assert_eq!(list.size() as usize, expected.len());
    assert_eq!(list.keys(), expected);
    for (index, key) in expected.iter().enumerate() {
        assert_eq!(list.get_key(index as u32), *key);
    }
    let mut copied = Vec::new();
    list.enum_keys(|key| copied.push(key));
    assert_eq!(copied, expected);
    // Callback values are copies; changing retained copies cannot alter owner.
    copied.fill(F);
    assert_eq!(list.keys(), expected);
}

#[test]
fn ordered_duplicates_and_three_word_equality() {
    let mut list = KeyList::new();
    sequence(&list, &[]);
    assert!(!list.is_present(A));
    assert!(!list.erase(A));
    for key in [A, T, A, G, I, Z, F] {
        assert!(list.insert(key));
    }
    sequence(&list, &[A, T, A, G, I, Z, F]);
    for key in [A, T, G, I, Z, F] {
        assert!(list.is_present(key));
    }
    assert!(!list.is_present(Key::new(9, 8, u32::MAX)));
    assert!(list.erase(A));
    sequence(&list, &[T, A, G, I, Z, F]);
    assert!(list.is_present(A));
    assert!(list.erase(A));
    sequence(&list, &[T, G, I, Z, F]);
    assert!(!list.is_present(A));
    assert!(!list.erase(A));
    sequence(&list, &[T, G, I, Z, F]);
}

#[test]
fn append_independent_list_in_enumeration_order() {
    let mut target = KeyList::new();
    let mut source = KeyList::new();
    target.insert(F);
    for key in [G, Z, G, T] {
        source.insert(key);
    }
    target.insert_list(&source);
    sequence(&target, &[F, G, Z, G, T]);
    sequence(&source, &[G, Z, G, T]);
    assert!(source.erase_all());
    source.insert(I);
    sequence(&target, &[F, G, Z, G, T]);
    target.insert_list(&source);
    sequence(&target, &[F, G, Z, G, T, I]);
    sequence(&source, &[I]);
}

#[test]
fn clear_empty_and_reuse() {
    let mut list = KeyList::new();
    assert!(list.erase_all());
    list.insert(A);
    list.insert(A);
    assert!(list.erase_all());
    sequence(&list, &[]);
    assert!(!list.is_present(A));
    assert!(!list.erase(A));
    list.insert(F);
    list.insert(Z);
    sequence(&list, &[F, Z]);
    list.insert_list(&KeyList::new());
    sequence(&list, &[F, Z]);
}
