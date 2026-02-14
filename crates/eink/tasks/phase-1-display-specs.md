# Phase 1: Foundation

**Duration:** Week 1 (20 hours)
**Status:** 📋 Planning
**Dependencies:** None

## Overview

Establish project structure, core traits, display specifications, and build infrastructure for the entire e-ink ecosystem.

---

## Tasks

### 1.1: Project Scaffolding
**Effort:** 2 hours | **Priority:** Critical | **Status:** ⏳ Pending

**Description:**
Create workspace structure for all five crates with proper Cargo.toml configurations.

**Acceptance Criteria:**
- [ ] Create `crates/eink/` directory structure
- [ ] Scaffold `eink-specs/` crate
- [ ] Scaffold `eink-system/` crate
- [ ] Scaffold `eink-components/` crate
- [ ] Scaffold `eink-emulator/` crate
- [ ] Scaffold `eink-testing/` crate
- [ ] Root `Cargo.toml` workspace configuration
- [ ] Each crate has `src/lib.rs`, `Cargo.toml`, `README.md`

**Implementation Notes:**
```bash
# Directory structure
crates/eink/
├─ eink-specs/
│  ├─ src/lib.rs
│  ├─ Cargo.toml
│  └─ README.md
├─ eink-system/
├─ eink-components/
├─ eink-emulator/
└─ eink-testing/
```

**Verification:**
```bash
cargo build --workspace
```

---

### 1.2: eink-specs Core Types
**Effort:** 3 hours | **Priority:** Critical | **Status:** ⏳ Pending

**Description:**
Define `DisplaySpec` struct and related types for describing e-ink display characteristics.

**Acceptance Criteria:**
- [ ] `DisplaySpec` struct with all fields
- [ ] `RefreshMode` enum (Full, Partial, Fast)
- [ ] `Controller` enum (SSD1680, IL0373, etc.)
- [ ] `PanelType` enum (Pearl, Carta1000, Carta1300, Kaleido3)
- [ ] `TemperatureRange` struct
- [ ] All types derive `Debug`, `Clone`, `Copy` where appropriate
- [ ] `#[cfg_attr(feature = "serde", derive(Deserialize))]` for TOML support
- [ ] Comprehensive rustdoc on all public types

**Implementation:**
```rust
// src/lib.rs
#![no_std]

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct DisplaySpec {
    pub name: &'static str,
    pub width: u32,
    pub height: u32,
    pub controller: Controller,
    pub panel_type: PanelType,
    pub refresh_modes: &'static [RefreshMode],
    pub temperature_range: TemperatureRange,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub enum RefreshMode {
    Full {
        duration_ms: u32,
        grayscale_levels: u8,
        flash_count: u8,
    },
    Partial {
        duration_ms: u32,
        grayscale_levels: u8,
        ghosting_rate: f32,
    },
    Fast {
        duration_ms: u32,
        grayscale_levels: u8,
        ghosting_rate: f32,
    },
}

// ... more types
```

**Verification:**
- [ ] Compiles with `--no-default-features`
- [ ] Compiles with `--features serde`
- [ ] Doc tests pass

---

### 1.3: Predefined Display Specs (Waveshare)
**Effort:** 2 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Define constants for popular Waveshare e-ink displays.

**Acceptance Criteria:**
- [ ] `displays::WAVESHARE_2_13_V4` (250×122, SSD1680)
- [ ] `displays::WAVESHARE_2_9_V2` (296×128, IL0373)
- [ ] `displays::WAVESHARE_4_2_V2` (400×300, SSD1619)
- [ ] `displays::WAVESHARE_7_5_V2` (800×480, ED075TC1)
- [ ] Each spec has accurate technical specifications
- [ ] Rustdoc with links to datasheets

**Implementation:**
```rust
// src/displays/waveshare.rs
pub const WAVESHARE_2_13_V4: DisplaySpec = DisplaySpec {
    name: "Waveshare 2.13\" V4",
    width: 250,
    height: 122,
    controller: Controller::SSD1680,
    panel_type: PanelType::Carta1000,
    refresh_modes: &[
        RefreshMode::Full {
            duration_ms: 2000,
            grayscale_levels: 16,
            flash_count: 3,
        },
        RefreshMode::Partial {
            duration_ms: 300,
            grayscale_levels: 4,
            ghosting_rate: 0.15,
        },
    ],
    temperature_range: TemperatureRange {
        optimal_min: 15,
        optimal_max: 35,
        operating_min: 0,
        operating_max: 50,
    },
};
```

**Verification:**
- [ ] All specs compile
- [ ] Example program uses each spec
- [ ] Documentation links to official Waveshare pages

---

### 1.4: Predefined Display Specs (GoodDisplay)
**Effort:** 2 hours | **Priority:** Medium | **Status:** ⏳ Pending

**Description:**
Define constants for popular GoodDisplay e-ink displays.

**Acceptance Criteria:**
- [ ] `displays::GDEY042T81` (4.2" fast refresh)
- [ ] `displays::GDEM075T41WT` (7.5" wide temp)
- [ ] `displays::GDEW0154M09` (1.54" high PPI)
- [ ] Each spec has accurate specifications

**Verification:**
- [ ] Compiles
- [ ] Documentation complete

---

### 1.5: Core Trait Definitions
**Effort:** 3 hours | **Priority:** Critical | **Status:** ⏳ Pending

**Description:**
Define core traits used across the ecosystem (View, Layout, DrawTarget extensions).

**Acceptance Criteria:**
- [ ] `View` trait in `eink-system`
- [ ] `Layout` trait in `eink-system`
- [ ] `TestId` trait in `eink-testing`
- [ ] All traits have comprehensive rustdoc
- [ ] Example implementations in doc comments

**Implementation:**
```rust
// eink-system/src/view.rs
use embedded_graphics::prelude::*;
use crate::{Style, Theme};

pub trait View {
    /// Compute layout for this view given constraints
    fn layout(&self, constraints: Constraints, theme: &Theme) -> LayoutNode;

    /// Draw this view to the target
    fn draw<D>(&self, target: &mut D, bounds: Rectangle) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray4>;

    /// Get test ID if set (for testing)
    fn test_id(&self) -> Option<&str> {
        None
    }
}

// eink-system/src/layout.rs
pub trait Layout {
    fn compute(&self, constraints: Constraints) -> Size;
}
```

**Verification:**
- [ ] Traits compile in `no_std`
- [ ] Doc tests demonstrate usage

---

### 1.6: CI/CD Pipeline Setup
**Effort:** 4 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Set up GitHub Actions for continuous integration and deployment.

**Acceptance Criteria:**
- [ ] `.github/workflows/ci.yml` created
- [ ] Format check (`cargo fmt --check`)
- [ ] Linting (`cargo clippy -- -D warnings`)
- [ ] Tests (`cargo test --all-features`)
- [ ] no_std build verification
- [ ] std build verification
- [ ] Cache cargo dependencies
- [ ] Run on PR and main branch

**Implementation:**
```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main]
    paths:
      - 'crates/eink/**'
  pull_request:
    paths:
      - 'crates/eink/**'

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable

      - name: Format
        run: cargo fmt --check --manifest-path crates/eink/Cargo.toml

      - name: Clippy
        run: cargo clippy --manifest-path crates/eink/Cargo.toml --all-features -- -D warnings

      - name: Test (std)
        run: cargo test --manifest-path crates/eink/Cargo.toml --all-features

      - name: Build (no_std)
        run: cargo build --manifest-path crates/eink/eink-system/Cargo.toml --no-default-features
```

**Verification:**
- [ ] CI passes on clean build
- [ ] CI fails on format violation
- [ ] CI fails on clippy warning
- [ ] CI fails on test failure

---

### 1.7: Workspace Dependencies Configuration
**Effort:** 2 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Configure workspace-level dependencies for version consistency.

**Acceptance Criteria:**
- [ ] `[workspace.dependencies]` section in root Cargo.toml
- [ ] All common dependencies defined at workspace level
- [ ] Member crates use `workspace = true` for shared deps
- [ ] Feature flags properly configured

**Implementation:**
```toml
# Root Cargo.toml
[workspace]
resolver = "2"
members = [
    "crates/eink/eink-specs",
    "crates/eink/eink-system",
    "crates/eink/eink-components",
    "crates/eink/eink-emulator",
    "crates/eink/eink-testing",
]

[workspace.dependencies]
embedded-graphics = "0.8"
heapless = "0.8"
winit = "0.30"
softbuffer = "0.4"
hot-lib-reloader = "0.8"
serde = { version = "1.0", features = ["derive"], default-features = false }
```

**Verification:**
- [ ] `cargo build --workspace` succeeds
- [ ] No duplicate dependency versions

---

### 1.8: Documentation Templates
**Effort:** 2 hours | **Priority:** Medium | **Status:** ⏳ Pending

**Description:**
Create README templates and documentation guidelines for all crates.

**Acceptance Criteria:**
- [ ] README.md template with standard sections
- [ ] CHANGELOG.md template
- [ ] Each crate has initial README
- [ ] Documentation guidelines in CONTRIBUTING.md

**README Sections:**
1. Overview
2. Features
3. Installation
4. Quick Start
5. Examples
6. Documentation
7. License
8. Contributing

**Verification:**
- [ ] All crates have README.md
- [ ] `cargo readme` generates consistent output

---

## Phase Completion Checklist

- [ ] All tasks completed
- [ ] All tests passing
- [ ] CI/CD pipeline green
- [ ] Code review complete
- [ ] Documentation reviewed
- [ ] Ready for Phase 2

---

**Dependencies for Next Phase:**
- Phase 2 (Core System) depends on:
  - Task 1.2 (Core trait definitions)
  - Task 1.5 (View/Layout traits)
  - Task 1.7 (Workspace config)

**Estimated Completion:** End of Week 1
