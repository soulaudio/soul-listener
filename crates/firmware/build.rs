fn main() {
    // Only run linker script setup for hardware builds
    #[cfg(feature = "hardware")]
    {
        use std::env;
        use std::fs::File;
        use std::io::Write;
        use std::path::PathBuf;

        // Put `memory.x` in our output directory and ensure it's on the linker search path.
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

        // Only link to device.x for hardware builds
        println!("cargo:rerun-if-changed=../../memory.x");
    }

    println!("cargo:rerun-if-changed=build.rs");
}
