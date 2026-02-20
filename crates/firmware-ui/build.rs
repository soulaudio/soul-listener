//! Build script for firmware-ui: ABI signature hash enforcement (GAP-M5).
//!
//! Computes a stable hash of the `render_ui` C-ABI signature string and emits
//! it as the `RENDER_UI_SIGNATURE_HASH` environment variable.
//!
//! # Why this exists
//!
//! `firmware-ui` exports `render_ui` via `#[no_mangle] pub unsafe extern "C"`.
//! The binary side loads the dylib at runtime and calls this function directly
//! through a raw function pointer — there is no Rust type system checking the
//! call site. If the signature changes (new parameter, different type) and
//! `ABI_VERSION` is not bumped, the binary loads the new `.so` with the old ABI
//! and gets undefined behaviour at runtime.
//!
//! This build script makes that forgotten bump *visible*:
//!   1. The signature string is hashed here at build time.
//!   2. The hash is embedded in the dylib via `RENDER_UI_SIGNATURE_HASH`.
//!   3. A CI job (or a `build.rs` assertion in the binary) compares the hash
//!      against the expected value. A mismatch surfaces in build output before
//!      any code runs.
//!
//! # Updating the signature
//!
//! When `render_ui`'s parameter list changes:
//!   1. Update the `RENDER_UI_SIGNATURE` constant below to match the new
//!      parameter types exactly.
//!   2. Bump `ABI_VERSION` in `src/lib.rs`.
//!   3. Update the binary side to pass the new parameters.
//!
//! The hash is FNV-1a 32-bit — stable, deterministic, no external deps.

fn main() {
    // This string must exactly mirror the parameter types of the `render_ui`
    // function in src/lib.rs. Update it whenever the signature changes, and
    // bump ABI_VERSION at the same time.
    let render_ui_signature =
        "render_ui(emulator_ptr: *mut eink_emulator::Emulator) -> ()";

    let hash = fnv1a_32(render_ui_signature);
    println!("cargo:rustc-env=RENDER_UI_SIGNATURE_HASH={hash:08x}");

    // Re-run this build script if the lib source changes (signature may change).
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=build.rs");
}

/// FNV-1a 32-bit hash — stable, deterministic, zero external dependencies.
///
/// Reference: <http://www.isthe.com/chongo/tech/comp/fnv/>
fn fnv1a_32(s: &str) -> u32 {
    const FNV_OFFSET_BASIS: u32 = 2_166_136_261;
    const FNV_PRIME: u32 = 16_777_619;

    let mut hash = FNV_OFFSET_BASIS;
    for byte in s.bytes() {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}
