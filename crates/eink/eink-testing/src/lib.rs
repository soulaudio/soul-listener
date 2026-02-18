//! E-Ink Testing Utilities
//!
//! Playwright-like testing API for the e-ink emulator.

use std::path::Path;

pub use eink_emulator::Emulator;
pub use eink_specs::DisplaySpec;

#[derive(Debug, Clone, PartialEq)]
pub struct ComponentRef {
    pub test_id: String,
    pub component_type: String,
    pub position: (i32, i32),
    pub size: (u32, u32),
}

pub struct TestEmulator {
    inner: eink_emulator::Emulator,
}

impl TestEmulator {
    pub fn new(width: u32, height: u32) -> Self {
        Self { inner: eink_emulator::Emulator::headless(width, height) }
    }

    pub fn with_spec(spec: &'static eink_specs::DisplaySpec) -> Self {
        Self { inner: eink_emulator::Emulator::headless_with_spec(spec) }
    }

    pub fn query_by_test_id(&self, test_id: &str) -> Option<ComponentRef> {
        #[cfg(feature = "debug")]
        {
            let dm = self.inner.debug_manager()?;
            let comp = dm.state().registered_components.iter()
                .find(|c| c.test_id.as_deref() == Some(test_id))?;
            return Some(ComponentRef {
                test_id: comp.test_id.clone().unwrap_or_else(|| test_id.to_string()),
                component_type: comp.component_type.clone(),
                position: comp.position,
                size: comp.size,
            });
        }
        #[allow(unreachable_code)]
        { let _ = test_id; None }
    }

    pub fn query_all(&self) -> Vec<ComponentRef> {
        #[cfg(feature = "debug")]
        {
            let Some(dm) = self.inner.debug_manager() else { return Vec::new(); };
            return dm.state().registered_components.iter().map(|c| ComponentRef {
                test_id: c.test_id.clone().unwrap_or_default(),
                component_type: c.component_type.clone(),
                position: c.position,
                size: c.size,
            }).collect();
        }
        #[allow(unreachable_code)]
        Vec::new()
    }

    pub fn component_count(&self) -> usize {
        #[cfg(feature = "debug")]
        {
            return self.inner.debug_manager()
                .map(|dm| dm.state().registered_components.len())
                .unwrap_or(0);
        }
        #[allow(unreachable_code)]
        0
    }

    pub fn screenshot(&self, path: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
        self.inner.screenshot(path)
    }

    pub fn assert_matches_screenshot(
        &self,
        golden_path: impl AsRef<Path>,
        threshold: u8,
    ) -> Result<(), String> {
        use image::GenericImageView;
        let tmp_path = {
            let mut p = std::env::temp_dir();
            p.push(format!("eink_testing_cmp_{}.png", std::process::id()));
            p
        };
        self.inner.screenshot(&tmp_path)
            .map_err(|e| format!("Failed to capture screenshot: {e}"))?;
        let current = image::open(&tmp_path)
            .map_err(|e| format!("Failed to open temp screenshot: {e}"))?;
        let _ = std::fs::remove_file(&tmp_path);
        let golden = image::open(golden_path.as_ref())
            .map_err(|e| format!("Failed to open golden screenshot: {e}"))?;
        if current.dimensions() != golden.dimensions() {
            let (cw, ch) = current.dimensions();
            let (gw, gh) = golden.dimensions();
            return Err(format!(
                "Screenshot dimensions mismatch: current {cw}x{ch} vs golden {gw}x{gh}"
            ));
        }
        let current_rgba = current.to_rgba8();
        let golden_rgba = golden.to_rgba8();
        let mut diff_count: u64 = 0;
        for (cp, gp) in current_rgba.pixels().zip(golden_rgba.pixels()) {
            let differs = cp.0.iter().zip(gp.0.iter()).any(|(&a, &b)| {
                (a as i32 - b as i32).unsigned_abs() as u8 > threshold
            });
            if differs { diff_count += 1; }
        }
        if diff_count > 0 {
            Err(format!("{diff_count} pixels differ from golden"))
        } else {
            Ok(())
        }
    }

    pub fn emulator(&self) -> &eink_emulator::Emulator { &self.inner }
    pub fn emulator_mut(&mut self) -> &mut eink_emulator::Emulator { &mut self.inner }
}

impl std::ops::Deref for TestEmulator {
    type Target = eink_emulator::Emulator;
    fn deref(&self) -> &Self::Target { &self.inner }
}

impl std::ops::DerefMut for TestEmulator {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.inner }
}
