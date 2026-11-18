# eink-ui ui-1 — core model + tokens + golden harness — Implementation Plan

> REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use `- [ ]`.
> Spec: `docs/plans/2026-11-16-eink-ui-toolkit-design.md`. This is ui-1 of 4 (flexbox layout lands in ui-1 Task 2; widgets in ui-2; e-ink refresh ui-3; adoption ui-4).

**Goal:** Stand up the standalone `eink-ui` crate with its core immediate-mode model (`Widget` trait + `Constraints`/`Size`), the Tailwind-like `Theme`/tokens, `Text` + `Container`, and a host golden-test harness — then the flexbox `Row`/`Col`/`Stack`/`Spacer` engine. All no_std + no-alloc, rendering to any `DrawTarget<Color=Gray4>`.

**Architecture:** A top-level `eink-ui/` crate (its own workspace, so it lifts out to a separate repo with a `git mv`). Depends on **only** `embedded-graphics` + `heapless`, with optional `alloc`/`std` features. Zero soul-listener coupling.

**Tech Stack:** Rust `#![no_std]` `#![forbid(unsafe_code)]`, `embedded-graphics` 0.8, `heapless`, `image` (host test feature only).

## Global Constraints

- **Deps are ONLY** `embedded-graphics`, `heapless`; `alloc` + `std` are optional features (`std` enables the golden-test framebuffer+PNG). NOTHING else — no embassy, no soul/dap crates, ever.
- No-alloc in the default build; const-generics / slices for bounded composition. `#![forbid(unsafe_code)]`.
- Must build for host AND `thumbv7em-none-eabihf` (no_std).
- Commits `sebastianstupak <sebastian.stupak@pm.me>`, Conventional Commits, no AI trailers (hooks enforce). Branch `feat/eink-ui`. Run `cargo fmt` + clippy-clean before committing.

---

### Task 1: Crate scaffold + `Widget` model + `Theme`/tokens + `Text`/`Container` + golden harness

**Files:** Create `eink-ui/Cargo.toml`, `eink-ui/src/lib.rs`, `eink-ui/src/widget.rs`, `eink-ui/src/theme.rs`, `eink-ui/src/widgets/{text,container}.rs`, `eink-ui/src/testing.rs` (std-gated), `eink-ui/tests/golden.rs`.

**Interfaces — Produces:**
- `widget::{Widget, Constraints, Size, DrawError}`:
  ```rust
  pub struct Constraints { pub min: Size, pub max: Size }   // Size = { w: u32, h: u32 }
  pub trait Widget {
      fn measure(&self, limits: Constraints, theme: &Theme) -> Size;
      fn draw<D: DrawTarget<Color = Gray4>>(&self, d: &mut D, rect: Rectangle, theme: &Theme) -> Result<(), DrawError<D::Error>>;
  }
  ```
- `theme::{Theme, Space, TextScale, Role}` — `Space::{Xs..Xl}→u32`, `TextScale::{Caption,Body,Title,Display}`→an `embedded-graphics` `MonoFont`, `Role::{Ink,Muted,Faint,Paper}`→`Gray4`. `Theme::default()`.
- `widgets::{Text, Container}` — `Text::new(&str).scale(TextScale).role(Role)`; `Container::new(child).padding(Space).background(Option<Role>).border(Option<Role>)` (generic over the child `Widget`, no Box in the no-alloc path).
- `testing::Framebuffer` (std-gated) — an in-memory `DrawTarget<Color=Gray4>` of a given size with `save_png(path)` + `coverage()`/`pixel()`.

- [ ] **Step 1:** `eink-ui/Cargo.toml` (`[package] name="eink-ui"`, `[features] default=[]`, `alloc=[]`, `std=["alloc","dep:image"]`; deps: `embedded-graphics`, `heapless`, optional `image`). `eink-ui/src/lib.rs`: `#![no_std]`, `#![forbid(unsafe_code)]`, `#[cfg(feature="std")] extern crate std;`, module decls.
- [ ] **Step 2:** `widget.rs` — the trait + `Size`/`Constraints`/`DrawError`.
- [ ] **Step 3:** `theme.rs` — the token enums + `Theme` + `Default`. Map `TextScale` to real `embedded_graphics::mono_font::ascii` fonts (e.g. `FONT_6X10`/`FONT_10X20`); `Role` to `Gray4` values (Ink=0x0, Muted=0x5, Faint=0xA, Paper=0xF).
- [ ] **Step 4:** `testing.rs` (`#[cfg(feature="std")]`) — the `Framebuffer` `DrawTarget<Gray4>` + PNG via `image`.
- [ ] **Step 5 (TDD):** `tests/golden.rs` (run with `--features std`) — render a `Text` and a `Container{Text}` to a `Framebuffer`, assert measured `Size` is sane and the framebuffer is non-blank with ink where expected; `save_png` to `eink-ui/target/golden/` for eyeballing. Write failing first, implement `Text`/`Container` until green.
- [ ] **Step 6 (verify):** `cd eink-ui && cargo test --features std` (golden green) · `cargo build` (no_std, no-alloc) · `cargo build --target thumbv7em-none-eabihf` · `cargo clippy --all-targets --all-features -- -D warnings`.
- [ ] **Step 7:** Commit `feat(eink-ui): crate + Widget model + Theme/tokens + Text/Container + golden harness`.

---

### Task 2: Flexbox layout engine (`Row`/`Col`/`Stack`/`Spacer`)

**Files:** Create `eink-ui/src/layout.rs`, `eink-ui/src/widgets/{stack,spacer}.rs`; modify `lib.rs`, `tests/golden.rs`.

**Interfaces — Produces:** `layout::{Row, Col, Stack, Spacer, Justify, Align, FlexChild}` + a `flex(children, main_axis, justify, align, gap, padding, limits)` algorithm. `Row`/`Col` hold children as a fixed-capacity `heapless::Vec<FlexChild, N>` or a const-generic array; each `FlexChild` wraps a `&dyn`-free child via an enum or a generic tuple (no_std + no-alloc: prefer a builder taking `impl Widget` items into a bounded array, or a macro `row![a, b, c]`). Per-child `grow`/`shrink`/`basis`.

- [ ] **Step 1:** `layout.rs` — `Justify`/`Align` enums + the flex algorithm: measure each child under the cross-axis limit, sum main-axis basis + gaps, distribute remaining main space by `grow` (or shrink overflow by `shrink`), then place each child's `Rectangle` per `justify` (main) + `align` (cross, incl. `Stretch`), honoring `padding`.
- [ ] **Step 2:** `Row`/`Col` widgets (thin wrappers setting the main axis) + `Stack` (z-overlay, all children same rect) + `Spacer`/`Flex(n)` (grow-only empty). Keep the no-alloc child-holding ergonomic (a `row!`/`col!` macro building a bounded tuple/array is fine).
- [ ] **Step 3 (TDD):** extend `tests/golden.rs` — a `Row` of three `Text`s with `gap` + `Justify::SpaceBetween`: assert the three computed child rects are left/center/right with the gap, non-overlapping; a `Col` with a `grow` child that fills remaining height; a `Container{ padding, Row{...} }`. Golden PNGs saved. Failing first, implement until green.
- [ ] **Step 4 (verify):** `cd eink-ui && cargo test --features std` · `cargo build --target thumbv7em-none-eabihf` · clippy clean.
- [ ] **Step 5:** Commit `feat(eink-ui): flexbox layout engine (Row/Col/Stack/Spacer)`.

---

## Deliverable

A standalone `eink-ui` crate: the immediate-mode `Widget` model, Tailwind-like `Theme`/tokens, `Text`/`Container`, a **working flexbox engine** (the thing v1 lacked — children get real positions), and a golden-image test harness — no_std, no-alloc, host + thumbv7em, zero soul-listener coupling. Next: **ui-2** (Label/Button/ProgressBar/Icon/List), then **ui-3** (dirty-region refresh), then **ui-4** (adopt in soul-listener, delete v1 eink).
