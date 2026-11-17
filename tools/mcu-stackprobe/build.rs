//! Standard cortex-m-rt `memory.x` plumbing: copy it into `OUT_DIR` and add
//! that directory to the linker search path, so `link.x`'s `INCLUDE
//! memory.x` resolves to *this* crate's generic memory map, not some other
//! `memory.x` found elsewhere on the search path.

use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR set by cargo"));
    File::create(out_dir.join("memory.x"))
        .expect("create OUT_DIR/memory.x")
        .write_all(include_bytes!("memory.x"))
        .expect("write memory.x contents");
    println!("cargo:rustc-link-search={}", out_dir.display());
    println!("cargo:rerun-if-changed=memory.x");

    // The repo ROOT .cargo/config.toml's rustflags (`-Tlink.x -Tdefmt.x`) is
    // inherited via cargo's directory-walk config discovery — a different
    // mechanism from workspace membership, so this crate's own `[workspace]`
    // table doesn't stop it, and rustflags arrays MERGE across config files
    // (unlike scalar keys like `linker`, which this crate's own
    // .cargo/config.toml successfully overrides outright). This harness
    // doesn't depend on `defmt` (no `.defmt` sections exist to place), so no
    // real defmt.x is ever generated; without a stand-in, the inherited
    // `-Tdefmt.x` fails the link with "cannot find linker script defmt.x".
    // An empty linker script is a valid no-op `INCLUDE` target, so this
    // satisfies it harmlessly.
    File::create(out_dir.join("defmt.x")).expect("create OUT_DIR/defmt.x (stub, no defmt dep)");
}
