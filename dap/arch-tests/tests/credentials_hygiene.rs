//! Credentials must stay out of git, and the template must stay in step with the
//! docs.
//!
//! # What this gate does and does not read
//!
//! It reads `.gitignore` and `.env.example` — both committed — and the two
//! sourcing documents. **It never reads `.env`.** A test that opened the real
//! credential file would pull secrets into its own failure output and into CI
//! logs, which is a worse outcome than the leak it is trying to prevent. The
//! question "is `.env` ignored?" is answerable from `.gitignore` alone.
//!
//! # The failure modes
//!
//! Two, and the second is the sneaky one:
//!
//! 1. `.env` stops being ignored — a reordered `.gitignore`, or someone adding
//!    `!.env` while debugging — and the next `git add -A` commits live
//!    credentials. Irreversible in a public repo: a force-push does not unring
//!    a key that has been fetched.
//! 2. The template and the documentation drift. A new distributor variable gets
//!    added to the docs and not to `.env.example`, so whoever follows the README
//!    sets four of five values and the fetcher fails with a confusing auth error
//!    rather than "you did not set this one".

use std::collections::BTreeSet;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repo root must be two levels above dap/arch-tests")
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

/// The ignore patterns that apply to a path, in file order.
///
/// Returns `(line_no, pattern, is_negation)` for each match. Order matters:
/// gitignore resolves a path by **last match wins**, so a negation after a
/// wildcard un-ignores, and a wildcard after a negation re-ignores.
fn matching_patterns(gitignore: &str, path: &str) -> Vec<(usize, String, bool)> {
    let mut out = Vec::new();
    for (i, raw) in gitignore.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Strip an inline comment the way git does not — this repo's .gitignore
        // uses trailing comments (`hardware/.ato/   # cache`), so the pattern is
        // the first whitespace-delimited token.
        let pattern = line.split_whitespace().next().unwrap_or(line);
        let (negated, pat) = match pattern.strip_prefix('!') {
            Some(p) => (true, p),
            None => (false, pattern),
        };
        if glob_matches(pat, path) {
            out.push((i + 1, pat.to_string(), negated));
        }
    }
    out
}

/// Enough of gitignore glob semantics for the patterns this gate cares about:
/// a literal name, or one `*` wildcard.
fn glob_matches(pattern: &str, path: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == path,
        Some((prefix, suffix)) => {
            path.len() >= prefix.len() + suffix.len()
                && path.starts_with(prefix)
                && path.ends_with(suffix)
        }
    }
}

/// Is this path ignored? Last matching pattern wins.
fn is_ignored(gitignore: &str, path: &str) -> bool {
    matching_patterns(gitignore, path)
        .last()
        .map(|(_, _, negated)| !negated)
        .unwrap_or(false)
}

/// `.env` is ignored; `.env.example` is not.
///
/// Both halves matter. An over-broad rule that swallowed the template would be
/// safe but would leave nobody able to discover which variables to set.
#[test]
fn the_credential_file_is_ignored_and_the_template_is_not() {
    let gi = read(".gitignore");

    assert!(
        is_ignored(&gi, ".env"),
        "`.env` is NOT ignored by .gitignore.\n\n\
         The next `git add -A` would commit live distributor credentials, and in a public repo \
         that is irreversible — a force-push does not unring a key somebody already fetched. \
         Patterns currently matching `.env`: {:?}",
        matching_patterns(&gi, ".env")
    );

    // The variants people actually create.
    for variant in [".env.local", ".env.production", ".env.tme"] {
        assert!(
            is_ignored(&gi, variant),
            "`{variant}` is not ignored. People do create these; the `.env.*` wildcard is what \
             covers them."
        );
    }

    assert!(
        !is_ignored(&gi, ".env.example"),
        "`.env.example` is ignored, so the committed template is invisible.\n\n\
         The negation must come AFTER the `.env.*` wildcard — gitignore resolves by last match \
         wins, so order is the whole mechanism here. Patterns matching it: {:?}",
        matching_patterns(&gi, ".env.example")
    );
}

/// The template exists and holds no values.
///
/// A committed template with a value in it is a committed credential, whether or
/// not the value is real — and nobody reviewing a diff can tell a placeholder
/// from a live key by looking.
#[test]
fn the_template_is_committed_and_carries_no_values() {
    let tpl = read(".env.example");

    for (i, line) in tpl.lines().enumerate() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let Some((key, value)) = t.split_once('=') else {
            panic!(
                ".env.example:{} is neither a comment nor a KEY=VALUE line: {t:?}",
                i + 1
            );
        };
        let value = value.trim();

        // One deliberate exception: the Digi-Key host is a hostname, not a
        // secret, and defaulting it to the sandbox is the safe first run.
        if key.trim() == "DIGIKEY_API_HOST" {
            assert!(
                value.starts_with("sandbox-"),
                ".env.example defaults DIGIKEY_API_HOST to {value:?}. It should default to the \
                 sandbox host: the flow is identical, and a first run against production spends \
                 real quota while the field mapping is still unproven."
            );
            continue;
        }

        assert!(
            value.is_empty(),
            ".env.example:{} gives `{}` a value ({value:?}).\n\n\
             The template is COMMITTED, so a value here is a committed credential — and a \
             reviewer cannot tell a placeholder from a live key by reading a diff. Leave it empty \
             and describe it in the comment above.",
            i + 1,
            key.trim()
        );
    }
}

/// Variable names in the template, the docs and the README must agree.
///
/// The drift this catches is quiet and annoying rather than dangerous: a new
/// variable documented but absent from the template means whoever follows the
/// README sets every value they can see and still gets an auth failure, with
/// nothing pointing at the one they never knew about.
#[test]
fn the_template_and_the_docs_name_the_same_variables() {
    let tpl = read(".env.example");
    let setup = read("docs/sourcing/DISTRIBUTOR-API-SETUP.md");
    let readme = read("docs/sourcing/README.md");

    let declared: BTreeSet<&str> = tpl
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once('=').map(|(k, _)| k.trim()))
        .collect();

    assert!(
        !declared.is_empty(),
        ".env.example declares no variables at all"
    );

    for var in &declared {
        assert!(
            setup.contains(*var),
            "`{var}` is in .env.example but not in DISTRIBUTOR-API-SETUP.md. A variable with no \
             documented source is one nobody can fill in."
        );
        assert!(
            readme.contains(*var),
            "`{var}` is in .env.example but the sourcing README does not mention it, so following \
             the README leaves it unset — and the failure surfaces as a confusing auth error \
             rather than as a missing value."
        );
    }

    // And the other direction: a variable the setup doc exports must be in the
    // template. Picked up from the `export FOO=` lines the doc uses.
    for line in setup.lines() {
        let t = line.trim();
        let Some(rest) = t.strip_prefix("export ") else {
            continue;
        };
        let Some((key, _)) = rest.split_once('=') else {
            continue;
        };
        let key = key.trim();
        assert!(
            declared.contains(key),
            "DISTRIBUTOR-API-SETUP.md tells the reader to `export {key}=...`, but .env.example \
             does not declare it. Whoever fills in the template will miss it entirely."
        );
    }
}

/// No credential-shaped value is committed anywhere in the sourcing tree.
///
/// A backstop rather than a primary defence — the primary defence is `.gitignore`
/// and the empty template. This catches the other route in: a key pasted into a
/// doc, a snapshot note, or a CI workflow while debugging.
#[test]
fn no_committed_sourcing_file_contains_a_credential_assignment() {
    const FILES: &[&str] = &[
        ".env.example",
        "docs/sourcing/README.md",
        "docs/sourcing/DISTRIBUTOR-API-SETUP.md",
        "hardware/bom/snapshot.json",
        "hardware/bom/catalogue.fixture.json",
    ];
    const KEYS: &[&str] = &[
        "TME_TOKEN",
        "TME_APP_SECRET",
        "MOUSER_SEARCH_API_KEY",
        "DIGIKEY_CLIENT_ID",
        "DIGIKEY_CLIENT_SECRET",
    ];

    for f in FILES {
        let body = read(f);
        for (i, line) in body.lines().enumerate() {
            for key in KEYS {
                // An assignment with something after the `=` is the shape of a
                // leak. Mentions of the name, and empty assignments, are fine —
                // the docs necessarily do both.
                let Some(pos) = line.find(key) else { continue };
                let after = &line[pos + key.len()..];
                let Some(rest) = after.strip_prefix('=') else {
                    continue;
                };
                let value = rest.trim().trim_matches('"').trim_matches('`');
                // `export FOO=...` in the docs is an instruction, not a value.
                let is_placeholder = value.is_empty()
                    || value.starts_with("...")
                    || value.starts_with('#')
                    || value.starts_with('<');
                assert!(
                    is_placeholder,
                    "{f}:{} assigns `{key}` the value {value:?}.\n\n\
                     This file is committed. If that is a real credential it is already in the \
                     history and must be ROTATED at the distributor — removing the line is not \
                     enough. If it is a placeholder, leave the assignment empty or write it as \
                     `{key}=...`.",
                    i + 1
                );
            }
        }
    }
}

/// The glob and last-match-wins logic this gate depends on, pinned.
///
/// Worth its own test because `git check-ignore`'s exit code is misleading here:
/// it exits 0 when any pattern matches, **including a negation**, so
/// `check-ignore .env.example` succeeds on a correctly-configured repo and looks
/// like the template is ignored. This gate reasons about pattern order instead,
/// and that reasoning needs to be right.
#[test]
fn last_match_wins_and_the_wildcard_only_matches_what_it_should() {
    let gi = "\
.env
.env.*
!.env.example
";
    assert!(is_ignored(gi, ".env"));
    assert!(is_ignored(gi, ".env.local"));
    assert!(!is_ignored(gi, ".env.example"), "the negation must win");

    // Order reversed: the wildcard now comes last and re-ignores the template.
    let reversed = "\
.env
!.env.example
.env.*
";
    assert!(
        is_ignored(reversed, ".env.example"),
        "with the negation first, the wildcard re-ignores the template — which is exactly the \
         ordering mistake the main test guards against, so this logic must model it"
    );

    // The wildcard must not swallow unrelated names.
    assert!(!is_ignored(gi, "environment"));
    assert!(!is_ignored(gi, "src/.envoy"));
}
