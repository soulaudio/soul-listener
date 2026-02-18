//! Debug state management

use std::collections::HashSet;
use std::time::Instant;

/// Box-model spacing (margin / border / padding) in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Spacing {
    pub top: u16,
    pub right: u16,
    pub bottom: u16,
    pub left: u16,
}

impl Spacing {
    pub const fn all(v: u16) -> Self {
        Self { top: v, right: v, bottom: v, left: v }
    }
    pub const fn axes(vert: u16, horiz: u16) -> Self {
        Self { top: vert, right: horiz, bottom: vert, left: horiz }
    }
    pub fn is_zero(&self) -> bool {
        self.top == 0 && self.right == 0 && self.bottom == 0 && self.left == 0
    }
}

/// Component information for debugging
#[derive(Debug, Clone, Default)]
pub struct ComponentInfo {
    pub component_type: String,
    pub position: (i32, i32),
    pub size: (u32, u32),
    pub test_id: Option<String>,
    /// Outer margin (outside the border).
    pub margin: Spacing,
    /// Inner padding (inside the border).
    pub padding: Spacing,
    /// Border width on each side.
    pub border: Spacing,
    /// Arbitrary key-value attributes for display in the CMP inspector tab.
    pub attributes: Vec<(String, String)>,
}

impl ComponentInfo {
    pub fn with_margin(mut self, s: Spacing) -> Self {
        self.margin = s;
        self
    }
    pub fn with_padding(mut self, s: Spacing) -> Self {
        self.padding = s;
        self
    }
    pub fn with_border(mut self, s: Spacing) -> Self {
        self.border = s;
        self
    }
    pub fn with_attr(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.attributes.push((k.into(), v.into()));
        self
    }
}

/// Power consumption sample
#[derive(Debug, Clone, Copy)]
pub struct PowerSample {
    pub timestamp: Instant,
    pub power_mw: f32,
    pub refresh_type: Option<RefreshType>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshType {
    Full,
    Partial,
    Fast,
}

/// Which tab is currently active in the debug side panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugTab {
    /// Scene hierarchy tree + component inspector (default).
    Scene,
    /// Display dimensions, rotation, temperature, refresh counts, hotkeys.
    Display,
    /// Live power graph and battery statistics.
    Power,
}

/// A single row in the flattened, visibility-resolved scene tree.
///
/// Built by [`DebugState::build_scene_rows`].  Collapsed subtrees are omitted
/// so the caller can display the list directly without extra filtering.
#[derive(Debug, Clone)]
pub struct SceneRow {
    /// Index into [`DebugState::registered_components`].
    pub comp_idx: usize,
    /// Nesting depth (0 = root, 1 = direct child of root, …).
    pub depth: usize,
    /// Whether this node has at least one child in the scene.
    pub has_children: bool,
    /// Whether this node is currently collapsed (children hidden).
    pub is_collapsed: bool,
    /// True when this row represents a virtual label-group header rather than
    /// a real component.  `comp_idx` points to the first label in the group.
    pub is_label_group: bool,
    /// Number of labels merged into this group (meaningful only when
    /// `is_label_group` is true).
    pub label_group_count: usize,
}

/// Debug system state
pub struct DebugState {
    pub panel_visible: bool,
    pub borders_enabled: bool,
    pub inspector_mode: bool,
    pub hovered_component: Option<ComponentInfo>,
    pub selected_component: Option<ComponentInfo>,
    pub power_history: Vec<PowerSample>, // Will be ring buffer later
    /// Explicitly registered components shown by the borders overlay.
    /// When empty, the overlay falls back to auto-generated display regions.
    pub registered_components: Vec<ComponentInfo>,
    /// Number of full refreshes recorded
    pub full_refresh_count: u64,
    /// Number of partial refreshes recorded
    pub partial_refresh_count: u64,
    /// Currently active tab in the debug panel.
    pub active_tab: DebugTab,
    /// Set of component bounding-box keys whose subtrees have been explicitly
    /// **expanded** by the user.  Nodes with children are collapsed by default;
    /// adding a node to this set reveals its children.  Key = `(x, y, w, h)`.
    pub expanded_nodes: HashSet<(i32, i32, u32, u32)>,
    /// Currently keyboard-selected row index in the scene tree (index into the
    /// result of [`build_scene_rows`]).  `None` = nothing selected.
    pub scene_selected: Option<usize>,
    /// First visible row index (scroll offset) for the scene tree.
    pub scene_scroll: usize,
}

impl Default for DebugState {
    fn default() -> Self {
        Self {
            panel_visible: false,
            borders_enabled: false,
            inspector_mode: false,
            hovered_component: None,
            selected_component: None,
            power_history: Vec::new(),
            registered_components: Vec::new(),
            full_refresh_count: 0,
            partial_refresh_count: 0,
            active_tab: DebugTab::Scene,
            expanded_nodes: HashSet::new(),
            scene_selected: None,
            scene_scroll: 0,
        }
    }
}

impl DebugState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance to the next tab (Scene → Display → Power → Scene).
    pub fn cycle_tab(&mut self) {
        self.active_tab = match self.active_tab {
            DebugTab::Scene   => DebugTab::Display,
            DebugTab::Display => DebugTab::Power,
            DebugTab::Power   => DebugTab::Scene,
        };
    }

    pub fn toggle_panel(&mut self) {
        self.panel_visible = !self.panel_visible;
    }

    pub fn toggle_borders(&mut self) {
        self.borders_enabled = !self.borders_enabled;
    }

    pub fn toggle_inspector(&mut self) {
        self.inspector_mode = !self.inspector_mode;
    }

    /// Register a component so it appears in the borders overlay (Ctrl+2).
    ///
    /// When at least one component is registered the overlay draws only the
    /// registered components.  When no components are registered the overlay
    /// falls back to auto-generated display-bound regions.
    pub fn register_component(&mut self, component: ComponentInfo) {
        self.registered_components.push(component);
    }

    /// Remove all registered components, reverting to the fallback display regions.
    pub fn clear_registered_components(&mut self) {
        self.registered_components.clear();
        self.expanded_nodes.clear();
        self.scene_selected = None;
        self.scene_scroll = 0;
    }

    /// Record a full refresh event, incrementing the full refresh counter.
    pub fn record_full_refresh(&mut self) {
        self.full_refresh_count += 1;
    }

    /// Record a partial refresh event, incrementing the partial refresh counter.
    pub fn record_partial_refresh(&mut self) {
        self.partial_refresh_count += 1;
    }

    /// Return the bounding-box key used to look up a component in
    /// `expanded_nodes`.
    fn node_key(comp: &ComponentInfo) -> (i32, i32, u32, u32) {
        (comp.position.0, comp.position.1, comp.size.0, comp.size.1)
    }

    /// Return `true` if `comp`'s subtree is currently collapsed.
    ///
    /// Nodes are **collapsed by default** (when not in `expanded_nodes`).
    /// Only nodes explicitly expanded by the user show their children.
    pub fn is_node_collapsed(&self, comp: &ComponentInfo) -> bool {
        !self.expanded_nodes.contains(&Self::node_key(comp))
    }

    /// Toggle the collapsed/expanded state of `comp`.
    ///
    /// If the node is expanded, collapse it (remove from `expanded_nodes`).
    /// If the node is collapsed, expand it (add to `expanded_nodes`).
    pub fn toggle_node_collapsed(&mut self, comp: &ComponentInfo) {
        let key = Self::node_key(comp);
        if !self.expanded_nodes.remove(&key) {
            self.expanded_nodes.insert(key);
        }
    }

    /// Build the flat list of visible scene rows respecting collapsed state.
    ///
    /// Uses an iterative DFS starting from root nodes (components with no
    /// spatial parent).  Children of collapsed nodes are omitted.
    /// After the initial DFS, consecutive same-depth Label/Text leaves that
    /// share the same tree position are merged into virtual group rows when
    /// there are ≥ 3 of them.
    pub fn build_scene_rows(&self) -> Vec<SceneRow> {
        let comps = &self.registered_components;
        if comps.is_empty() {
            return Vec::new();
        }

        // --- iterative DFS ---------------------------------------------------
        // Find roots (no spatial parent) and sort top-to-bottom, left-to-right.
        let mut roots: Vec<usize> = (0..comps.len())
            .filter(|&i| self.find_parent(&comps[i]).is_none())
            .collect();
        roots.sort_by_key(|&i| (comps[i].position.1, comps[i].position.0));

        // Stack holds (comp_idx, depth); push in reverse so we pop in order.
        let mut stack: Vec<(usize, usize)> =
            roots.into_iter().rev().map(|i| (i, 0usize)).collect();

        let mut raw: Vec<SceneRow> = Vec::new();

        while let Some((idx, depth)) = stack.pop() {
            let comp = &comps[idx];
            let children = self.find_children(comp);
            let has_children = !children.is_empty();
            let is_collapsed = self.is_node_collapsed(comp);

            raw.push(SceneRow {
                comp_idx: idx,
                depth,
                has_children,
                is_collapsed,
                is_label_group: false,
                label_group_count: 0,
            });

            if !is_collapsed && has_children {
                let mut child_idxs: Vec<usize> = children
                    .iter()
                    .filter_map(|c| {
                        comps.iter().position(|cc| {
                            cc.position == c.position && cc.size == c.size
                        })
                    })
                    .collect();
                child_idxs
                    .sort_by_key(|&i| (comps[i].position.1, comps[i].position.0));
                for ci in child_idxs.into_iter().rev() {
                    stack.push((ci, depth + 1));
                }
            }
        }

        // --- label-group post-processing ------------------------------------
        // Replace runs of ≥ 3 same-depth text/label leaves with a group row.
        const MIN_GROUP: usize = 3;
        let mut result: Vec<SceneRow> = Vec::with_capacity(raw.len());
        let mut i = 0;

        while i < raw.len() {
            let row = &raw[i];
            let is_text_leaf = !row.has_children
                && !row.is_label_group
                && matches!(
                    comps[row.comp_idx].component_type.as_str(),
                    "Label" | "Text"
                );

            if is_text_leaf {
                let depth = row.depth;
                let mut j = i;
                while j < raw.len() {
                    let r = &raw[j];
                    let tl = !r.has_children
                        && !r.is_label_group
                        && matches!(
                            comps[r.comp_idx].component_type.as_str(),
                            "Label" | "Text"
                        );
                    if r.depth == depth && tl {
                        j += 1;
                    } else {
                        break;
                    }
                }

                let count = j - i;
                if count >= MIN_GROUP {
                    let first = &raw[i];
                    let group_collapsed =
                        self.is_node_collapsed(&comps[first.comp_idx]);
                    result.push(SceneRow {
                        comp_idx: first.comp_idx,
                        depth,
                        has_children: false,
                        is_collapsed: group_collapsed,
                        is_label_group: true,
                        label_group_count: count,
                    });
                    if !group_collapsed {
                        for k in i..j {
                            result.push(raw[k].clone());
                        }
                    }
                    i = j;
                    continue;
                }
            }

            result.push(row.clone());
            i += 1;
        }

        result
    }

    /// Find the immediate spatial parent of `target` in the registered scene.
    ///
    /// The parent is the smallest registered component whose bounding box fully
    /// encloses `target`.  Returns `None` for root (unconstrained) components.
    pub fn find_parent<'a>(&'a self, target: &ComponentInfo) -> Option<&'a ComponentInfo> {
        let tx  = target.position.0;
        let ty  = target.position.1;
        let tx2 = tx + target.size.0 as i32;
        let ty2 = ty + target.size.1 as i32;
        let target_area = target.size.0 as u64 * target.size.1 as u64;

        let mut best: Option<&ComponentInfo> = None;
        let mut best_area = u64::MAX;
        for comp in &self.registered_components {
            if comp.position == target.position && comp.size == target.size {
                continue; // skip self
            }
            let cx  = comp.position.0;
            let cy  = comp.position.1;
            let cx2 = cx + comp.size.0 as i32;
            let cy2 = cy + comp.size.1 as i32;
            let area = comp.size.0 as u64 * comp.size.1 as u64;
            // Parent must be strictly larger and fully contain the target.
            if area > target_area && cx <= tx && cy <= ty && cx2 >= tx2 && cy2 >= ty2 {
                if area < best_area {
                    best_area = area;
                    best = Some(comp);
                }
            }
        }
        best
    }

    /// Collect all **direct** spatial children of `parent`.
    ///
    /// A direct child is a registered component fully contained within `parent`
    /// that has no intermediate ancestor between itself and `parent`.
    pub fn find_children<'a>(&'a self, parent: &ComponentInfo) -> Vec<&'a ComponentInfo> {
        let px  = parent.position.0;
        let py  = parent.position.1;
        let px2 = px + parent.size.0 as i32;
        let py2 = py + parent.size.1 as i32;
        let parent_area = parent.size.0 as u64 * parent.size.1 as u64;

        let mut children: Vec<&ComponentInfo> = self
            .registered_components
            .iter()
            .filter(|comp| {
                if comp.position == parent.position && comp.size == parent.size {
                    return false;
                }
                let cx  = comp.position.0;
                let cy  = comp.position.1;
                let cx2 = cx + comp.size.0 as i32;
                let cy2 = cy + comp.size.1 as i32;
                let area = comp.size.0 as u64 * comp.size.1 as u64;
                area < parent_area && cx >= px && cy >= py && cx2 <= px2 && cy2 <= py2
            })
            .filter(|child| match self.find_parent(*child) {
                // Direct child: its immediate parent IS `parent`.
                Some(p) => p.position == parent.position && p.size == parent.size,
                None => false,
            })
            .collect();

        children.sort_by_key(|c| (c.position.1, c.position.0));
        children
    }
}
