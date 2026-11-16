# eink-ui — a reusable immediate-mode e-ink UI toolkit — Design Spec

**Date:** 2026-11-16
**Status:** Draft for review
**Context:** Replaces soul-listener's half-baked v1 eink toolkit (its flexbox discarded child positions; `Label` was `&'static str`-only) with a proper, standalone, publishable library. soul-listener becomes a consumer.

## 1. Vision

A **no_std, `embedded-graphics`-native, immediate-mode UI toolkit specialized for e-ink** — as expressive as CSS/Tailwind for layout + styling, but with zero heap by default. The embedded-Rust gap is real: `embedded-layout` has no true flex or styling; `egui`/`Slint`/`LVGL` are std/alloc-heavy or C-codegen and not e-ink-aware. **Hard separation:** depends on *only* `embedded-graphics` + `heapless` (+ optional `alloc`), knows nothing about soul-listener, and is extractable to its own repo + crates.io.

Working crate name: **`eink-ui`** (alternatives to decide at review: `quill`, `slate`, `graphein`). Lives in-repo at `eink-ui/` (its own crate), consumed by `dap-core`'s screens.

## 2. Core model (decided)

- **Immediate-mode.** Each update you construct lightweight widget *values* and render them in a single **measure → layout → draw** pass. No retained tree, no diffing machinery, no persistent allocation. Fits e-ink's infrequent updates.
- **No-alloc core, `alloc` optional.** Bounded composition (const-generics / slices / `heapless`) with zero heap; an `alloc` feature unlocks unbounded children + dynamic `String` text for desktop/large targets.
- **The `Widget` trait** (the whole contract):
  ```rust
  pub trait Widget {
      fn measure(&self, limits: Constraints, style: &Theme) -> Size;      // intrinsic/clamped size
      fn draw<D: DrawTarget<Color = Gray4>>(&self, d: &mut D, rect: Rectangle, style: &Theme) -> Result<(), DrawError<D>>;
  }
  ```
  Containers call `measure` on children, run the flex algorithm, then `draw` each into its computed `Rectangle`. Leaves (Text, ProgressBar…) draw directly. Children are held by value/slice — no boxing in the no-alloc path.

## 3. Layer 1 — Layout (flexbox)

A real flexbox pass (the thing v1 lacked): `Row`/`Col` containers with `justify` (Start/Center/End/SpaceBetween/SpaceAround), `align` (Start/Center/End/Stretch), per-child `grow`/`shrink`/`basis`, `gap`, `padding`, `margin`. `Stack` (z-overlay) and `Spacer`/`Flex(n)` too. The algorithm: measure children under the cross-axis constraint, distribute free main-axis space by grow/shrink, place by justify/align. Fixed-capacity children (const-generic `N`) in the no-alloc path; `Vec` under `alloc`.

## 4. Layer 2 — Style tokens (the "Tailwind" part)

Ergonomic Rust tokens, not CSS strings:
- **Spacing scale** (`Space::{Xs,Sm,Md,Lg,Xl}` → px) for padding/gap/margin.
- **Typography scale** (`Text::{Caption,Body,Title,Display}`) mapping to `embedded-graphics` fonts (+ an optional scaler for Display sizes).
- **Color roles** in `Gray4` (`Ink` = darkest, `Muted`, `Faint`, `Paper` = lightest) — semantic, not raw gray values, so a `Theme` can remap them (e.g. inverted selection).
- **`Theme`** — a swappable struct bundling the scales + roles + default widget styling, threaded through `measure`/`draw`. A `Theme::default()` plus easy overrides.
- A chainable `Style` on widgets (`.padding(Space::Md).grow(1).align(Center)`).

## 5. Layer 3 — Widgets (v1 set)

`Text`/`Label` (**dynamic** via `&str` + an `alloc`-optional owned variant), `Row`/`Col`/`Stack`/`Spacer`, `Container` (padding/border/background), `Button` (label + pressed state, draws the inverted/boxed look), `ProgressBar`, `Icon` (from a small built-in glyph set + user bitmaps), `List`/`ListItem` (the browse primitive: rows, a selected index → highlight, a scroll indicator). Composable; all pure `Widget`s.

## 6. Layer 4 — the e-ink specialization

What makes it an *e-ink* toolkit, not generic:
- **`Gray4`** throughout (4-gray), with the color-role system tuned for e-ink contrast.
- **Dirty-region tracking** — the renderer records the bounding box of what changed vs the last frame, so the driver can do a **partial refresh** of just that region (e-ink's key perf feature). v1 ships region-granular (whole-widget bounding boxes); per-pixel diff is a later refinement.
- **Refresh hints** — widgets/screens can request `Full` (ghost-clear) vs `Partial` refresh; a periodic full refresh to clear ghosting is a caller policy the toolkit surfaces.
- Everything renders to any `DrawTarget<Color = Gray4>` — the same code hits soul-listener's `SimDisplay` (host) and the real SSD1677 panel.

## 7. Testing (reusable crate standard)

`eink-ui` has its own host test suite: render widgets/layouts to an in-memory `Gray4` framebuffer and assert via **golden images** (a tiny framebuffer+PNG helper, gated behind a `std`/test feature) + structural assertions (measured sizes, computed child rects). This is the toolkit's own proof, independent of soul-listener.

## 8. Hard-separation guarantees

- `eink-ui/Cargo.toml` deps: `embedded-graphics`, `heapless`; optional `alloc` + `std` (test/PNG) features. **Nothing else**, and nothing soul-listener.
- A dependency-boundary test (in soul-listener CI) fails if `eink-ui` ever gains a `dap`/soul dependency.
- Public API documented with examples; `#![no_std]`, `#![forbid(unsafe_code)]`.
- Extraction path: `git mv eink-ui <new-repo>` + publish — nothing in it references the monorepo.

## 9. soul-listener adoption (closes the debt)

Rewrite `dap-core`'s `now_playing` screen to use `eink-ui` (replacing the raw-`embedded-graphics` hand-placement), proving the API on a real screen and deleting the "half-baked toolkit" finding. The v1 `crates/eink/*` become dead and are removed (a later cleanup), except `eink-specs` panel data if still useful and the SSD1677 driver (salvaged for the board adapter).

## 10. Phasing

1. **ui-1 — core:** the crate, `Widget`/`Constraints`/`Size`, the flexbox `Row`/`Col`/`Stack`/`Spacer` + the layout algorithm, `Theme` + tokens, and `Text`/`Container` — with golden tests for layout.
2. **ui-2 — widgets:** `Label`/`Button`/`ProgressBar`/`Icon`/`List` + more golden tests.
3. **ui-3 — e-ink layer:** dirty-region tracking + refresh hints.
4. **ui-4 — adopt:** rewrite soul-listener's Now-Playing (and later Browse) on `eink-ui`; add the dep-boundary CI test; remove the dead v1 eink crates.

## 11. Out of scope (v1)

Grid layout, images/animation, text input/IME, a retained/diffing mode, theming hot-reload. All addable later without breaking the `Widget` contract.
