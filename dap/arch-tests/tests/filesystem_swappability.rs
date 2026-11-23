//! The filesystem must stay swappable.
//!
//! `docs/research/2026-11-18-licensing-ip-and-workarounds.md` §6 commits to
//! "FAT32 now, exFAT swappable later behind an FS trait". That is a **licensing**
//! commitment as much as an architectural one: exFAT is still patented, so the
//! ability to add it later — or to move to littlefs, or to read a different
//! on-card layout — is what keeps the decision reversible instead of permanent.
//!
//! # The good news, and why these gates exist anyway
//!
//! The architecture already satisfies this. `dap-core` defines one
//! [`Storage`](dap_core::ports::Storage) port, keyed on a **track index**, and has
//! no dependency on `dap-storage-fat32` or on `embedded-sdmmc`. Seven types
//! implement that port and only one of them is FAT32-backed.
//!
//! So nothing here is a fix. These gates stop it from rotting, and the way it
//! would rot is specific and gradual: someone needs a filename, so the port grows
//! a `&str` path; then something needs a directory listing; then a cluster size
//! leaks into a buffer calculation. Each step is individually reasonable and the
//! end state is a port that only FAT32 can implement, discovered when someone
//! tries to add the second filesystem and finds the abstraction was never real.
//!
//! The strongest gate here is
//! [`the_storage_port_has_a_non_filesystem_implementation`] — not a rule about
//! what the code may contain, but a standing proof that a non-FAT32 storage
//! backend actually compiles against the port.

/// Crates and modules whose source is scanned for filesystem leakage.
const CORE_SOURCES: &[(&str, &str)] = &[
    ("core/src/ports.rs", include_str!("../../core/src/ports.rs")),
    ("core/src/audio.rs", include_str!("../../core/src/audio.rs")),
    ("core/src/app.rs", include_str!("../../core/src/app.rs")),
    (
        "core/src/firmware.rs",
        include_str!("../../core/src/firmware.rs"),
    ),
    ("core/src/lib.rs", include_str!("../../core/src/lib.rs")),
    ("core/src/stub.rs", include_str!("../../core/src/stub.rs")),
];

const CORE_MANIFEST: &str = include_str!("../../core/Cargo.toml");

/// Vocabulary that only exists because a *particular* filesystem does.
///
/// `cluster`, `VolumeIdx` and `ShortFileName` are FAT concepts; `inode` and
/// `superblock` belong to other families. Any of them appearing in `dap-core`
/// means the abstraction has sprung a leak in a specific direction.
const FILESYSTEM_VOCABULARY: &[&str] = &[
    "embedded_sdmmc",
    "VolumeIdx",
    "ShortFileName",
    "LongFileName",
    "FatVolume",
    "cluster_size",
    "ShortName",
    "superblock",
    "inode",
    "exfat",
    "exFAT",
    "littlefs",
];

/// Crate names that *are* a filesystem. `dap-core` may depend on none of them.
const FILESYSTEM_CRATES: &[&str] = &[
    "storage-fat32",
    "storage_fat32",
    "embedded-sdmmc",
    "fatfs",
    "littlefs",
    "exfat",
    "ext4",
];

/// Non-comment source lines, so a gate never fires on its own documentation.
fn code_lines(src: &str) -> impl Iterator<Item = (usize, &str)> {
    src.lines().enumerate().filter_map(|(i, l)| {
        let t = l.trim_start();
        if t.starts_with("//") || t.starts_with("/*") || t.starts_with('*') || t.starts_with("#!") {
            None
        } else {
            Some((i + 1, t))
        }
    })
}

/// `dap-core` must not depend on a filesystem.
///
/// The cheapest and most load-bearing of these checks: as long as the core cannot
/// *name* FAT32, it cannot accidentally be written against it.
#[test]
fn dap_core_does_not_depend_on_any_filesystem() {
    // Only the dependency sections, so a comment mentioning storage-fat32 in the
    // manifest is not a finding.
    for (line_no, line) in code_lines(CORE_MANIFEST) {
        for fs in FILESYSTEM_CRATES {
            assert!(
                !line.contains(fs),
                "core/Cargo.toml:{line_no} depends on '{fs}'.\n\n\
                 dap-core must stay filesystem-agnostic: it defines the `Storage` port and the \
                 board binds a concrete implementation to it. A dependency here is how the \
                 abstraction stops being one — and exFAT is still patented, so keeping the \
                 filesystem replaceable is a licensing position, not only a tidiness one.\n\n\
                 Put the dependency in board-h7a3 (the composition root) or in dap-sim (tests) \
                 instead.\n\nLine: {line}"
            );
        }
    }
}

/// No filesystem vocabulary in the core, including the port's own signature.
#[test]
fn no_filesystem_vocabulary_leaks_into_dap_core() {
    for (file, src) in CORE_SOURCES {
        for (line_no, line) in code_lines(src) {
            for term in FILESYSTEM_VOCABULARY {
                assert!(
                    !line.contains(term),
                    "{file}:{line_no} mentions '{term}', which is a concept belonging to one \
                     specific filesystem.\n\n\
                     dap-core talks to storage through the `Storage` port and must not know what \
                     is behind it. This is the first step of the slow failure these gates exist \
                     for: the vocabulary arrives before the dependency does.\n\nLine: {line}"
                );
            }
        }
    }
}

/// The port is addressed by **index**, never by path.
///
/// This is the design property that makes the abstraction real rather than
/// nominal. `open(&mut self, track: usize)` can be satisfied by a FAT32 card, a
/// RAM slice, a desktop file tree or a flash key-value store. The moment it
/// becomes `open(&mut self, path: &str)` it carries a filesystem's naming rules —
/// separators, case folding, length limits, encoding — and only something
/// filesystem-shaped can implement it.
#[test]
fn the_storage_port_is_addressed_by_index_not_by_path() {
    let ports = CORE_SOURCES
        .iter()
        .find(|(f, _)| *f == "core/src/ports.rs")
        .map(|(_, s)| *s)
        .expect("ports.rs must be in the scanned set");

    // Isolate the `trait Storage { .. }` body.
    let start = ports
        .find("pub trait Storage")
        .expect("dap-core must define a Storage port");
    let body: String = ports[start..]
        .lines()
        .take_while(|l| !l.starts_with('}'))
        .collect::<Vec<_>>()
        .join("\n");

    for banned in ["&str", "&Path", "Path,", "PathBuf", "&CStr", "filename"] {
        assert!(
            !body.contains(banned),
            "the Storage port's surface mentions '{banned}'.\n\n\
             A path- or name-addressed port inherits a filesystem's naming rules and can only be \
             implemented by something filesystem-shaped. Index addressing is what lets a RAM \
             fixture, an SD card and a future flash layout all satisfy the same trait.\n\n\
             If a name really is needed, resolve it behind the port (the implementor already owns \
             the index-to-location step) rather than widening the port.\n\nTrait body:\n{body}"
        );
    }

    assert!(
        body.contains("usize"),
        "the Storage port no longer takes a usize track index. If the addressing scheme changed \
         deliberately, this gate needs rewriting around the new scheme — but check first that the \
         new one can still be satisfied by a non-filesystem backend.\n\nTrait body:\n{body}"
    );
}

/// **The existence proof.** A non-filesystem backend compiles against the port.
///
/// Every other gate here is a prohibition, and prohibitions can all pass while
/// the abstraction is quietly unusable. This one is positive evidence: more than
/// one type implements `Storage`, and at least one of them has nothing to do with
/// a filesystem — so the port demonstrably does not require FAT32 semantics.
///
/// If this ever drops to "only the FAT32 one", swappability has become a claim
/// rather than a fact, and the next person to try adding exFAT finds out.
#[test]
fn the_storage_port_has_a_non_filesystem_implementation() {
    const IMPLEMENTORS: &[(&str, &str, bool)] = &[
        (
            "core/src/stub.rs",
            include_str!("../../core/src/stub.rs"),
            false,
        ),
        (
            "sim/src/storage.rs",
            include_str!("../../sim/src/storage.rs"),
            false,
        ),
        (
            "storage-fat32/src/lib.rs",
            include_str!("../../storage-fat32/src/lib.rs"),
            true,
        ),
    ];

    let mut filesystem_backed = 0usize;
    let mut other_backed: Vec<&str> = Vec::new();

    for (file, src, is_fs) in IMPLEMENTORS {
        let implements = src.contains("Storage for");
        assert!(
            implements,
            "{file} is listed as a Storage implementor but no longer implements the port. Update \
             this list — it is the swappability evidence, and silently losing an entry is how the \
             evidence evaporates."
        );
        if *is_fs {
            filesystem_backed += 1;
        } else {
            other_backed.push(file);
        }
    }

    assert!(
        !other_backed.is_empty(),
        "every Storage implementation is filesystem-backed. The port is then only nominally an \
         abstraction: nothing proves a different storage model can satisfy it, which is exactly \
         the state 'FAT32 is swappable' claims not to be in."
    );
    assert!(
        filesystem_backed >= 1,
        "no filesystem-backed Storage implementation exists, so the firmware cannot read a card. \
         That is a different problem, but this gate would otherwise pass vacuously."
    );

    println!(
        "Storage port: {} filesystem-backed, {} other ({:?}) — swappable",
        filesystem_backed,
        other_backed.len(),
        other_backed
    );
}

/// Only the composition root may name the concrete filesystem.
///
/// `board-h7a3` wiring FAT32 to the port is the whole point of a composition
/// root. The failure this catches is a *second* place doing it — a feature slice
/// reaching for the concrete type because it needed something the port does not
/// expose, which is how one dependency becomes a web of them.
#[test]
fn only_the_board_and_tests_bind_the_concrete_filesystem() {
    const NON_ROOT_SOURCES: &[(&str, &str)] = &[
        ("core/src/audio.rs", include_str!("../../core/src/audio.rs")),
        ("core/src/app.rs", include_str!("../../core/src/app.rs")),
        (
            "core/src/firmware.rs",
            include_str!("../../core/src/firmware.rs"),
        ),
        ("core/src/power.rs", include_str!("../../core/src/power.rs")),
        (
            "core/src/display.rs",
            include_str!("../../core/src/display.rs"),
        ),
    ];

    for (file, src) in NON_ROOT_SOURCES {
        for (line_no, line) in code_lines(src) {
            assert!(
                !line.contains("SdStorage") && !line.contains("storage_fat32"),
                "{file}:{line_no} names the concrete FAT32 storage type.\n\n\
                 Binding a filesystem to the port is the board's job (board-h7a3) — that is what \
                 a composition root is for. A feature slice naming it means the slice needed \
                 something the port does not offer; widen the port deliberately, or keep the need \
                 inside the implementor.\n\nLine: {line}"
            );
        }
    }
}
