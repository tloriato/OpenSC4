// SPDX-License-Identifier: GPL-3.0-or-later
//
// Link settings for fc4native.dll on 32-bit Windows (MSVC): no entry point, no default libraries (so no C runtime
// and no imports), relocations kept so any loader can place it, and a fixed preferred base below FreeCity4.dll
// (0x5C000000) and above the fixed schedule's heap region (0x20000000-0x50000000), so a game run with native
// functions loads it at one address every time. Other targets build an empty library.
//
// Only release builds are linked this way: the workspace's release profile aborts on panic, which is what the
// no-std DLL needs. Dev builds unwind, so they use std and link normally (see src/lib.rs); the game never loads
// them.
//
// It also turns held.tsv (ports kept out of the table until they are proven EXACT) into $OUT_DIR/held.rs.

use std::fmt::Write as _;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=held.tsv");
    let text = std::fs::read_to_string("held.tsv").expect("held.tsv");
    let mut held: Vec<u32> = text
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| u32::from_str_radix(l.split('\t').next().unwrap(), 16).expect("held.tsv address"))
        .collect();
    held.sort_unstable();
    held.dedup();
    let mut out = format!("pub const HELD: [u32; {}] = [", held.len());
    for a in &held {
        write!(out, "0x{a:08X},").unwrap();
    }
    out.push_str("];\n");
    let dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    std::fs::write(std::path::Path::new(&dir).join("held.rs"), out).expect("write held.rs");
    let target = std::env::var("TARGET").unwrap_or_default();
    let profile = std::env::var("PROFILE").unwrap_or_default();
    if target == "i686-pc-windows-msvc" && profile == "release" {
        for arg in ["/NOENTRY", "/NODEFAULTLIB", "/DYNAMICBASE:NO", "/BASE:0x5B000000"] {
            println!("cargo:rustc-cdylib-link-arg={arg}");
        }
    }
}
