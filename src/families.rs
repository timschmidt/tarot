//! Native toplevel identity, transient ownership and stacking, independent of cards.
use std::collections::HashMap;
use tarot_core::{Rect, WindowId};

#[derive(Clone, Default)]
pub struct Families {
    nodes: Vec<Node>,
    preferred: HashMap<WindowId, WindowId>,
    clock: u64,
}

#[derive(Clone, serde::Serialize)]
pub struct Node {
    pub id: WindowId,
    pub parent: Option<WindowId>,
    pub mapped: bool,
    pub modal: bool,
    order: u64,
}

impl Families {
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }
    pub fn register(&mut self, id: WindowId) {
        self.clock += 1;
        self.nodes.push(Node {
            id,
            parent: None,
            mapped: false,
            modal: false,
            order: self.clock,
        });
    }
    pub fn parent(&self, id: WindowId) -> Option<WindowId> {
        self.nodes.iter().find(|n| n.id == id)?.parent
    }
    pub fn mapped(&self, id: WindowId) -> bool {
        self.nodes.iter().any(|n| n.id == id && n.mapped)
    }
    pub fn set_parent(&mut self, id: WindowId, parent: Option<WindowId>) {
        // xdg-shell: an unmapped parent is equivalent to null. Smithay
        // rejects client cycles; retain a bounded defensive check here too.
        let parent = parent.filter(|p| self.mapped(*p) && !self.descends_from(*p, id));
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == id) {
            node.parent = parent;
        }
        self.prune_focus();
    }
    pub fn set_modal(&mut self, id: WindowId, modal: bool) {
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == id) {
            node.modal = modal;
        }
        if modal && self.mapped(id) {
            self.raise(id);
        }
    }
    pub fn map(&mut self, id: WindowId) {
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == id) {
            node.mapped = true;
        }
        self.raise(id);
        self.prefer(id);
    }
    pub fn unmap(&mut self, id: WindowId) {
        let parent = self.parent(id);
        for node in &mut self.nodes {
            if node.parent == Some(id) {
                node.parent = parent;
            }
            if node.id == id {
                node.parent = None;
                node.mapped = false;
            }
        }
        self.prune_focus();
    }
    pub fn remove(&mut self, id: WindowId) {
        self.unmap(id);
        self.nodes.retain(|n| n.id != id);
    }
    fn descends_from(&self, mut id: WindowId, ancestor: WindowId) -> bool {
        for _ in 0..=self.nodes.len() {
            if id == ancestor {
                return true;
            }
            let Some(parent) = self.parent(id) else {
                return false;
            };
            id = parent;
        }
        false
    }
    pub fn owner(&self, mut id: WindowId) -> Option<WindowId> {
        if !self.mapped(id) {
            return None;
        }
        for _ in 0..=self.nodes.len() {
            match self.parent(id) {
                Some(parent) => id = parent,
                None => return Some(id),
            }
        }
        None
    }
    pub fn roots(&self) -> Vec<WindowId> {
        self.nodes
            .iter()
            .filter(|n| n.mapped && n.parent.is_none())
            .map(|n| n.id)
            .collect()
    }
    /// Back to front; raising a branch never puts a child beneath an ancestor.
    pub fn members(&self, owner: WindowId) -> Vec<WindowId> {
        let mut out = Vec::new();
        let mut pending = vec![owner];
        while let Some(id) = pending.pop() {
            if !self.mapped(id) {
                continue;
            }
            out.push(id);
            let mut children: Vec<_> = self.nodes.iter().filter(|n| n.parent == Some(id)).collect();
            children.sort_by_key(|n| {
                (
                    self.nodes.iter().any(|m| {
                        m.mapped && m.modal && m.parent.is_some() && self.descends_from(m.id, n.id)
                    }),
                    n.order,
                )
            });
            pending.extend(children.into_iter().rev().map(|n| n.id));
        }
        out
    }
    fn modal(&self, owner: WindowId) -> Option<WindowId> {
        self.members(owner).into_iter().rev().find(|id| {
            self.nodes
                .iter()
                .any(|n| n.id == *id && n.modal && n.parent.is_some())
        })
    }
    pub fn accepts(&self, id: WindowId) -> bool {
        self.owner(id).is_some_and(|owner| {
            self.modal(owner)
                .is_none_or(|modal| self.descends_from(id, modal))
        })
    }
    pub fn focus(&self, owner: WindowId) -> Option<WindowId> {
        self.preferred
            .get(&owner)
            .copied()
            .filter(|id| self.owner(*id) == Some(owner) && self.accepts(*id))
            .or_else(|| {
                self.members(owner)
                    .into_iter()
                    .rev()
                    .find(|id| self.accepts(*id))
            })
    }
    pub fn prefer(&mut self, id: WindowId) {
        if !self.accepts(id) {
            return;
        }
        let Some(owner) = self.owner(id) else {
            return;
        };
        self.preferred.insert(owner, id);
        self.raise(id);
    }
    fn raise(&mut self, id: WindowId) {
        let mut branch = Some(id);
        while let Some(id) = branch {
            self.clock += 1;
            if let Some(node) = self.nodes.iter_mut().find(|n| n.id == id) {
                node.order = self.clock;
                branch = node.parent;
            } else {
                break;
            }
        }
    }
    fn prune_focus(&mut self) {
        let valid: Vec<_> = self
            .preferred
            .iter()
            .filter(|(owner, id)| self.owner(**id) == Some(**owner))
            .map(|(o, i)| (*o, *i))
            .collect();
        self.preferred = valid.into_iter().collect();
    }
}

/// A transient keeps its natural size, uniformly fitted into the available
/// app area, then follows its card's scale and center through every view.
pub fn transient_rect(card: Rect, content: Rect, width: i32, height: i32) -> Rect {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let fit = (content.width / w).min(content.height / h).min(1.0);
    let width = w * fit * card.width / content.width.max(1.0);
    let height = h * fit * card.height / content.height.max(1.0);
    Rect {
        x: card.x + (card.width - width) * 0.5,
        y: card.y + (card.height - height) * 0.5,
        width,
        height,
    }
}

impl crate::Compositor {
    pub fn native_window(&self, id: WindowId) -> Option<smithay::desktop::Window> {
        self.space
            .elements()
            .find(|w| w.user_data().get::<WindowId>() == Some(&id))
            .cloned()
    }
    pub fn native_id(
        &self,
        surface: &smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
    ) -> Option<WindowId> {
        self.space
            .elements()
            .find(|w| w.toplevel().is_some_and(|top| top.wl_surface() == surface))
            .and_then(|w| w.user_data().get::<WindowId>().copied())
    }
    pub fn focused_window(&self, owner: WindowId) -> Option<smithay::desktop::Window> {
        self.native_window(self.families.focus(owner)?)
    }
    pub fn native_rect(&self, id: WindowId, card: Rect) -> Rect {
        if self.families.parent(id).is_none() {
            return card;
        }
        let Some(window) = self.native_window(id) else {
            return card;
        };
        let size = window.geometry().size;
        transient_rect(card, self.window_area(id), size.w, size.h)
    }
    pub fn reconcile_families(&mut self, previous: Families, new_mapping: Option<WindowId>) {
        use smithay::wayland::{compositor::with_states, shell::xdg::XdgToplevelSurfaceData};
        use tarot_core::{Command, Effect, View};
        let old_view = self.shell.view;
        let old_selected = self.shell.selected;
        let roots = self.families.roots();
        // Keep the protocol role state coherent too. In particular, remapping
        // an old parent must never silently restore discarded parent edges.
        for node in self.families.nodes() {
            if let Some(window) = self.native_window(node.id)
                && let Some(top) = window.toplevel()
            {
                let parent = node
                    .parent
                    .and_then(|id| self.native_window(id))
                    .and_then(|w| w.toplevel().map(|t| t.wl_surface().clone()));
                with_states(top.wl_surface(), |states| {
                    states
                        .data_map
                        .get::<XdgToplevelSurfaceData>()
                        .unwrap()
                        .lock()
                        .unwrap()
                        .parent = parent;
                });
            }
        }
        for id in &roots {
            if let Some(window) = self.native_window(*id)
                && let Some(top) = window.toplevel()
            {
                let (title, app_id) = crate::handlers::xdg_shell::metadata(top);
                if !self.shell.cards.iter().any(|c| c.id == *id) {
                    self.shell.add_window_in_background(*id, title, app_id);
                }
            }
        }
        let removed: Vec<_> = self
            .shell
            .cards
            .iter()
            .filter(|c| !roots.contains(&c.id))
            .map(|c| c.id)
            .collect();
        for id in removed {
            self.shell.remove_window(id);
        }
        if let Some(selected) = old_selected.and_then(|id| self.families.owner(id)) {
            self.shell.selected = Some(selected);
        }
        if let View::App(old) = old_view {
            let target = self.families.owner(old).or_else(|| {
                roots
                    .iter()
                    .copied()
                    .find(|id| previous.owner(*id) == Some(old))
            });
            if let Some(target) = target {
                self.shell.view = View::App(target);
                self.shell.selected = Some(target);
                if target != old
                    && let Some(native) = previous.focus(old)
                {
                    self.families.prefer(native);
                }
            }
        }
        if let Some(id) = new_mapping
            && self.families.owner(id) == Some(id)
            && !self.protected()
            && !self.shell.system_menu_open
            && !self.shell.notifications_open
        {
            self.shell.apply(Command::Activate(id));
        }
        let active_family_changed = match self.shell.view {
            View::App(id) => {
                previous.members(id) != self.families.members(id)
                    || previous.focus(id) != self.families.focus(id)
            }
            _ => false,
        };
        if !self.protected()
            && (old_view != self.shell.view || previous.roots() != roots || active_family_changed)
        {
            self.cancel_input_sequences();
        }
        let focus = match self.shell.view {
            View::App(id) => Some(id),
            _ => None,
        };
        self.apply_effects(vec![Effect::Focus(focus)]);
        self.configure_windows();
        self.sync_app_menu();
        self.ui_dirty = true;
        self.redraw = true;
    }
    pub fn focus_native_surface(
        &mut self,
        surface: &smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
    ) {
        use tarot_core::{Effect, View};
        let Some(placement) = self.presented_surfaces.get(surface) else {
            return;
        };
        if placement.direct || self.protected() {
            return;
        }
        let id = placement.window;
        let Some(owner) = self.families.owner(id) else {
            return;
        };
        if self.shell.view != View::App(owner) || !self.families.accepts(id) {
            return;
        }
        let old = self.families.focus(owner);
        self.families.prefer(id);
        if self.families.focus(owner) != old {
            self.apply_effects(vec![Effect::Focus(Some(owner))]);
            self.redraw = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn orphan_inheritance_remap_and_modal_branches() {
        let mut f = Families::default();
        let [a, b, c, d] = [1, 2, 3, 4].map(WindowId);
        for id in [a, b, c, d] {
            f.register(id);
        }
        f.set_parent(b, Some(a)); // unmapped parent is ignored permanently
        f.map(a);
        f.map(b);
        assert_eq!(f.owner(b), Some(b));
        f.set_parent(b, Some(a));
        f.set_parent(c, Some(b));
        f.map(c);
        f.set_parent(d, Some(a));
        f.map(d);
        assert_eq!(f.roots(), vec![a]);
        f.set_modal(b, true);
        assert!(!f.accepts(a));
        assert!(!f.accepts(d));
        assert!(f.accepts(c));
        assert_eq!(f.focus(a), Some(c));
        assert_eq!(
            f.members(a),
            vec![a, d, b, c],
            "a modal branch stays above a later nonmodal sibling"
        );
        f.unmap(b);
        assert_eq!(f.parent(c), Some(a));
        f.map(b);
        assert_eq!(f.owner(b), Some(b));
        assert_eq!(f.parent(c), Some(a));
        f.remove(a);
        assert_eq!(f.owner(c), Some(c));
        assert_eq!(f.owner(d), Some(d));
    }
    #[test]
    fn nonmodal_focus_does_not_break_ancestor_stacking() {
        let mut f = Families::default();
        let [a, b, c] = [1, 2, 3].map(WindowId);
        for id in [a, b, c] {
            f.register(id);
            f.map(id);
        }
        f.set_parent(b, Some(a));
        f.set_parent(c, Some(b));
        f.prefer(a);
        assert_eq!(f.focus(a), Some(a));
        assert_eq!(f.members(a), vec![a, b, c]);
        f.set_parent(a, Some(c));
        assert_eq!(f.parent(a), None);
    }
    #[test]
    fn natural_dialog_size_and_oversized_aspect_fit_follow_the_card() {
        for content in [
            Rect {
                x: 0.0,
                y: 32.0,
                width: 360.0,
                height: 652.0,
            },
            Rect {
                x: 0.0,
                y: 32.0,
                width: 720.0,
                height: 292.0,
            },
        ] {
            let card = Rect {
                x: 61.25,
                y: 139.25,
                width: content.width * 0.66,
                height: content.height * 0.66,
            };
            let small = transient_rect(card, content, 80, 120);
            assert!((small.width - 52.8).abs() < 0.001 && (small.height - 79.2).abs() < 0.001);
            let big = transient_rect(card, content, 1200, 1800);
            assert!(big.width <= card.width + 0.001 && big.height <= card.height + 0.001);
            assert!((big.width / big.height - 2.0 / 3.0).abs() < 0.001);
            for r in [small, big] {
                assert!((r.x + r.width * 0.5 - card.x - card.width * 0.5).abs() < 0.001);
                assert!((r.y + r.height * 0.5 - card.y - card.height * 0.5).abs() < 0.001);
            }
        }
    }
}
