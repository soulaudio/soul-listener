fn main() {
    // Only run linker script setup for hardware builds
    #[cfg(feature = "hardware")]
    {
        use std::env;
        use std::fs::File;
        use std::io::Write;
        use std::path::PathBuf;

        // Put `memory.x` in our output directory and ensure it's on the linker search path.
        let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
        let memory_x = include_bytes!("../../memory.x");

        File::create(out.join("memory.x"))
            .unwrap()
            .write_all(memory_x)
            .unwrap();

        println!("cargo:rustc-link-search={}", out.display());

        // Only link to device.x for hardware builds
        println!("cargo:rerun-if-changed=../../memory.x");
    }

    println!("cargo:rerun-if-changed=build.rs");
}
