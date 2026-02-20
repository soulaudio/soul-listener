fn main() {
    // Rerun if linker scripts or this file change
    println!("cargo:rerun-if-changed=../../memory.x");
    println!("cargo:rerun-if-changed=build.rs");

    // Validate: `hardware` feature requires an ARM target.
    // CARGO_FEATURE_HARDWARE is set by Cargo when --features hardware is active.
    // CARGO_CFG_TARGET_ARCH is set to the target architecture (e.g. "arm", "x86_64").
    let hardware_feature = std::env::var("CARGO_FEATURE_HARDWARE").is_ok();
    if hardware_feature {
        let target_arch =
            std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| "unknown".to_string());
        if target_arch != "arm" {
            panic!(
                "firmware crate with `hardware` feature requires an ARM target \
                 (thumbv7em-none-eabihf), but CARGO_CFG_TARGET_ARCH=`{}`.\n\
                 Build with: cargo build --features hardware --target thumbv7em-none-eabihf",
                target_arch
            );
        }

        // Only run linker script setup for hardware builds.
        // Copy memory.x into the linker search path (OUT_DIR) so that
        // cortex-m-rt's link.x can INCLUDE it at link time.
        use std::env;
        use std::fs::File;
        use std::io::Write;
        use std::path::PathBuf;

        let out = PathBuf::from(
            env::var_os("OUT_DIR")
                .expect("OUT_DIR must be set by Cargo — this is a bug in the build system"),
        );
        let memory_x = include_bytes!("../../memory.x");

        File::create(out.join("memory.x"))
            .expect("failed to create memory.x in OUT_DIR")
            .write_all(memory_x)
            .expect("failed to write memory.x");

        println!("cargo:rustc-link-search={}", out.display());
    }
}
