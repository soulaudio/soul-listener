//! Put the crate-root `memory.x` on the linker search path.
//!
//! cortex-m-rt's `link.x` does `INCLUDE memory.x`, so `memory.x` must be
//! discoverable by the linker. We ship our own (see `memory.x`'s header for why
//! we don't use embassy-stm32's `memory-x` feature), so we copy it into `OUT_DIR`
//! and add that directory to the link search path — the canonical cortex-m-rt /
//! knurling `app-template` pattern, which both `rust-lld` and `flip-link` honor
//! (flip-link reads the `-L` paths when scanning linker scripts for the `RAM`
//! region). Without this, the ARM build fails with
//! `rust-lld: cannot find linker script memory.x`.
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR set by cargo"));
    File::create(out.join("memory.x"))
        .expect("create $OUT_DIR/memory.x")
        .write_all(include_bytes!("memory.x"))
        .expect("write memory.x");
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=build.rs");
}
