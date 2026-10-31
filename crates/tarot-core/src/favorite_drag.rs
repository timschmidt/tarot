//! Reversible favorite manipulation. Persistence occurs only at a valid release.
use crate::{
    Layout, Rect, ShellModel, View,
    quick_launch::{self, Edit, Favorites},
};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    Launcher(String),
    Dock { id: String, index: usize },
}

impl Source {
    pub fn id(&self) -> &str {
        match self {
            Self::Launcher(id) | Self::Dock { id, .. } => id,
        }
    }
    fn valid(&self, model: &ShellModel) -> bool {
        match self {
            Self::Launcher(id) => {
                model.view == View::Launcher
                    && !model.editing_favorites
                    && model.applications.iter().any(|a| a.id == *id)
            }
            Self::Dock { id, index } => {
                !matches!(model.view, View::App(_)) && model.favorites.at(*index) == Some(id)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    view: View,
    editing: bool,
    query: String,
    favorites: Favorites,
    catalog: Vec<(String, bool)>,
    pages: crate::launcher::Settings,
    banner: Option<(u32, u64)>,
    dashboard: bool,
    well_height: f32,
    size: (f32, f32),
}

impl Scene {
    pub fn new(model: &ShellModel, layout: Layout) -> Self {
        Self {
            view: model.view,
            editing: model.editing_favorites,
            query: model.launcher_query.clone(),
            favorites: model.favorites.clone(),
            pages: model.launcher.settings.clone(),
            size: (layout.width, layout.height),
            banner: model.notifications.banner,
            dashboard: model.notifications_open,
            well_height: layout.notification_well().height,
            catalog: model
                .applications
                .iter()
                .map(|a| (a.id.clone(), a.unavailable.is_none()))
                .collect(),
        }
    }
    pub fn allowed(model: &ShellModel) -> bool {
        !matches!(model.view, View::App(_))
            && !model.system_menu_open
            && !model.card_menu_open
            && model.network_input.is_none()
            && !model.launcher.menu_open
            && model.launcher.rename.is_none()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Place(usize),
    Remove,
    Full(usize),
    Unavailable(usize),
    Page {
        page: crate::launcher::PageId,
        at: usize,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Preview {
    pub source: Source,
    pub finger: (f32, f32),
    pub target: Option<Target>,
    pub favorites: Favorites,
    pub page: crate::launcher::PageId,
}

pub fn removal(layout: Layout) -> Rect {
    Rect {
        x: 16.0,
        y: quick_launch::slot(layout, 0, None).y - 54.0,
        width: (layout.width - 32.0).max(1.0),
        height: 44.0,
    }
}

#[derive(Clone, Debug)]
struct Contact {
    source: Source,
    scene: Scene,
    start: (f32, f32),
    finger: (f32, f32),
    pressed: u64,
    active: bool,
    moved: bool,
    page: crate::launcher::PageId,
    page_target: Option<(crate::launcher::PageId, usize)>,
    edge: Option<(i32, u64)>,
}

#[derive(Clone, Debug, Default)]
pub struct Gesture {
    contact: Option<Contact>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Release {
    pub consumed: bool,
    pub edit: Option<Edit>,
    pub page_edit: Option<crate::launcher::Edit>,
}

impl Gesture {
    pub fn in_progress(&self) -> bool {
        self.contact.is_some()
    }

    pub fn begin(
        &mut self,
        source: Source,
        finger: (f32, f32),
        now: u64,
        model: &ShellModel,
        layout: Layout,
    ) {
        self.contact = (Scene::allowed(model)
            && source.valid(model)
            && finger.0.is_finite()
            && finger.1.is_finite())
        .then(|| Contact {
            source,
            scene: Scene::new(model, layout),
            start: finger,
            finger,
            pressed: now,
            active: false,
            moved: false,
            page: model.launcher.settings.selected,
            page_target: None,
            edge: None,
        });
    }
    pub fn active(&self) -> bool {
        self.contact.as_ref().is_some_and(|c| c.active)
    }
    pub fn page(&self) -> Option<crate::launcher::PageId> {
        self.contact.as_ref().map(|c| c.page)
    }
    pub fn cancel(&mut self) {
        self.contact = None;
    }
    pub fn reconcile(&mut self, model: &ShellModel, layout: Layout) {
        if self.contact.as_ref().is_some_and(|c| {
            !Scene::allowed(model) || c.scene != Scene::new(model, layout) || !c.source.valid(model)
        }) {
            self.cancel();
        }
    }
    pub fn motion(&mut self, finger: (f32, f32)) {
        if let Some(c) = &mut self.contact {
            if !finger.0.is_finite()
                || !finger.1.is_finite()
                || (!c.active
                    && (finger.0 - c.start.0)
                        .abs()
                        .max((finger.1 - c.start.1).abs())
                        > crate::design::TAP_SLOP)
            {
                self.cancel();
            } else {
                c.moved |= (finger.0 - c.start.0)
                    .abs()
                    .max((finger.1 - c.start.1).abs())
                    > crate::design::TAP_SLOP;
                c.finger = finger;
            }
        }
    }
    pub fn tick(&mut self, now: u64) {
        if let Some(c) = &mut self.contact {
            c.active |= now.saturating_sub(c.pressed) >= crate::design::REORDER_HOLD_MS;
            let direction = if c.active
                && c.moved
                && matches!(c.source, Source::Launcher(_))
                && c.finger.1 >= Layout::STATUS + 64.0
                && c.finger.1 < c.scene.size.1 - Layout::GESTURE - crate::design::DOCK - 12.0
            {
                if c.finger.0 < 24.0 {
                    Some(-1)
                } else if c.finger.0 > c.scene.size.0 - 24.0 {
                    Some(1)
                } else {
                    None
                }
            } else {
                None
            };
            match (direction, c.edge) {
                (Some(side), Some((old, since))) if side == old => {
                    if now.saturating_sub(since) >= crate::launcher::EDGE_DWELL_MS {
                        if let Some(page) = c.scene.pages.neighbor(c.page, side) {
                            c.page = page;
                            c.page_target = None;
                        }
                        c.edge = Some((side, now));
                    }
                }
                (Some(side), _) => c.edge = Some((side, now)),
                _ => c.edge = None,
            }
        }
    }
    pub fn page_target(&mut self, target: Option<(crate::launcher::PageId, usize)>) {
        if let Some(c) = &mut self.contact {
            c.page_target = target.filter(|(page, _)| *page == c.page);
        }
    }
    pub fn wake_after(&self, now: u64) -> Option<Duration> {
        self.contact.as_ref().and_then(|c| {
            let deadline = if !c.active {
                c.pressed.saturating_add(crate::design::REORDER_HOLD_MS)
            } else if let Some((_, since)) = c.edge {
                since.saturating_add(crate::launcher::EDGE_DWELL_MS)
            } else {
                return None;
            };
            Some(Duration::from_millis(deadline.saturating_sub(now).max(1)))
        })
    }
    pub fn preview(&self, model: &ShellModel, layout: Layout) -> Option<Preview> {
        let c = self.contact.as_ref().filter(|c| c.active)?;
        let (x, y) = c.finger;
        // Revealing the tray must not turn a stationary hold on a low launcher
        // row into a drop merely because the tray appeared beneath the finger.
        // Returning to the original coordinates after a deliberate move,
        // including crossing onto another page, is still a moved gesture.
        let stationary_launcher = matches!(c.source, Source::Launcher(_)) && !c.moved;
        let target = if stationary_launcher {
            None
        } else if matches!(c.source, Source::Dock { .. }) && removal(layout).contains(x, y) {
            Some(Target::Remove)
        } else {
            (0..=quick_launch::FIFTH)
                .find(|i| quick_launch::slot(layout, *i, None).contains(x, y))
                .map(|i| {
                    if model
                        .applications
                        .iter()
                        .any(|a| a.id == c.source.id() && a.unavailable.is_some())
                    {
                        Target::Unavailable(i)
                    } else if i < quick_launch::FIFTH
                        && !model
                            .favorites
                            .applications
                            .iter()
                            .any(|id| id == c.source.id())
                        && model.favorites.applications.len() == quick_launch::CAPACITY
                    {
                        Target::Full(i)
                    } else {
                        Target::Place(i)
                    }
                })
        }
        .or_else(|| {
            if stationary_launcher || !matches!(c.source, Source::Launcher(_)) {
                return None;
            }
            c.page_target.map(|(page, at)| Target::Page { page, at })
        });
        let mut favorites = model.favorites.clone();
        if let Some(edit) = edit(&c.source, target) {
            favorites.edit(edit, &model.applications);
        }
        Some(Preview {
            source: c.source.clone(),
            finger: c.finger,
            target,
            favorites,
            page: c.page,
        })
    }
    pub fn release(&mut self, now: u64, model: &ShellModel, layout: Layout) -> Release {
        self.reconcile(model, layout);
        self.tick(now);
        let preview = self.preview(model, layout);
        self.cancel();
        let page_edit = preview.as_ref().and_then(|p| match p.target {
            Some(Target::Page { page, at }) => Some(crate::launcher::Edit::Place {
                application: p.source.id().into(),
                page,
                at,
            }),
            _ => None,
        });
        Release {
            consumed: preview.is_some(),
            edit: preview.and_then(|p| edit(&p.source, p.target)),
            page_edit,
        }
    }
}

fn edit(source: &Source, target: Option<Target>) -> Option<Edit> {
    match target {
        Some(Target::Place(at)) => Some(Edit::Place {
            id: source.id().to_owned(),
            at,
        }),
        Some(Target::Remove) => match source {
            Source::Dock { index, .. } => Some(Edit::Remove(*index)),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Application, Command, Effect};
    fn fixture() -> (ShellModel, Layout) {
        let mut model = ShellModel {
            view: View::Launcher,
            ..Default::default()
        };
        for n in 0..5 {
            model.applications.push(Application {
                id: format!("app{n}.desktop"),
                name: format!("App {n}"),
                description: String::new(),
                icon: None,
                keywords: vec![],
                unavailable: None,
            });
        }
        model.favorites.applications = vec!["app0.desktop".into(), "app1.desktop".into()];
        (
            model,
            Layout {
                width: 360.0,
                height: 720.0,
                notification_height: 0.0,
                keyboard_height: 0.0,
            },
        )
    }
    fn armed(model: &ShellModel, layout: Layout, source: Source) -> Gesture {
        let mut gesture = Gesture::default();
        gesture.begin(source, (100.0, 180.0), 0, model, layout);
        gesture.tick(crate::design::REORDER_HOLD_MS);
        assert!(gesture.active());
        gesture
    }
    fn center(layout: Layout, index: usize) -> (f32, f32) {
        let r = quick_launch::slot(layout, index, None);
        (r.x + r.width / 2.0, r.y + r.height / 2.0)
    }
    #[test]
    fn revealing_the_tray_beneath_a_stationary_launcher_hold_cannot_add_a_favorite() {
        let (model, layout) = fixture();
        let point = center(layout, 1);
        let mut g = Gesture::default();
        g.begin(
            Source::Launcher("app2.desktop".into()),
            point,
            0,
            &model,
            layout,
        );
        g.tick(500);
        assert_eq!(g.preview(&model, layout).unwrap().target, None);
        assert_eq!(
            g.release(600, &model, layout),
            Release {
                consumed: true,
                edit: None,
                page_edit: None,
            }
        );
    }

    #[test]
    fn hold_preview_is_reversible_and_release_persists_one_identity_once() {
        let (mut model, layout) = fixture();
        let before = model.favorites.clone();
        let mut g = armed(&model, layout, Source::Launcher("app2.desktop".into()));
        g.motion(center(layout, 0));
        let p = g.preview(&model, layout).unwrap();
        assert_eq!(
            p.favorites.applications,
            ["app2.desktop", "app0.desktop", "app1.desktop"]
        );
        assert_eq!(model.favorites, before);
        let release = g.release(700, &model, layout);
        assert!(release.consumed);
        let effects = model.apply(Command::Favorite(release.edit.unwrap()));
        assert_eq!(effects, vec![Effect::Favorites(p.favorites)]);
        assert_eq!(g.release(701, &model, layout), Release::default());
    }
    #[test]
    fn short_taps_and_pre_hold_scroll_never_become_favorite_edits() {
        let (model, layout) = fixture();
        let mut g = Gesture::default();
        g.begin(
            Source::Launcher("app2.desktop".into()),
            (100.0, 180.0),
            20,
            &model,
            layout,
        );
        assert_eq!(
            g.wake_after(20),
            Some(Duration::from_millis(crate::design::REORDER_HOLD_MS))
        );
        assert_eq!(g.release(100, &model, layout), Release::default());
        g.begin(
            Source::Launcher("app2.desktop".into()),
            (100.0, 180.0),
            0,
            &model,
            layout,
        );
        g.motion((100.0, 220.0));
        g.tick(1000);
        assert!(!g.active());
        assert_eq!(g.release(1100, &model, layout), Release::default());
    }
    #[test]
    fn full_tray_rejects_new_entry_but_existing_identity_can_move() {
        let (mut model, layout) = fixture();
        model
            .favorites
            .applications
            .extend(["app2.desktop".into(), "app3.desktop".into()]);
        let before = model.favorites.clone();
        let mut g = armed(&model, layout, Source::Launcher("app4.desktop".into()));
        g.motion(center(layout, 1));
        assert_eq!(
            g.preview(&model, layout).unwrap().target,
            Some(Target::Full(1))
        );
        assert!(g.release(800, &model, layout).edit.is_none());
        assert_eq!(model.favorites, before);
        let mut g = armed(&model, layout, Source::Launcher("app0.desktop".into()));
        g.motion(center(layout, 3));
        let edit = g.release(800, &model, layout).edit.unwrap();
        model.apply(Command::Favorite(edit));
        assert_eq!(
            model.favorites.applications,
            [
                "app1.desktop",
                "app2.desktop",
                "app3.desktop",
                "app0.desktop"
            ]
        );
        assert!(model.favorites.valid());
        assert!(!model.favorites.edit(
            Edit::Place {
                id: "app4.desktop".into(),
                at: 4
            },
            &model.applications
        ));
    }
    #[test]
    fn only_dock_sources_can_use_removal_and_apps_slot_never_accepts_a_drop() {
        let (model, layout) = fixture();
        let r = removal(layout);
        let point = (r.x + r.width / 2.0, r.y + r.height / 2.0);
        let source = Source::Dock {
            id: "app0.desktop".into(),
            index: 0,
        };
        let mut g = armed(&model, layout, source.clone());
        g.motion(point);
        assert_eq!(g.release(800, &model, layout).edit, Some(Edit::Remove(0)));
        let mut g = armed(&model, layout, Source::Launcher("app2.desktop".into()));
        g.motion(point);
        assert!(g.release(800, &model, layout).edit.is_none());
        let mut g = armed(&model, layout, source);
        g.motion(center(layout, 4));
        assert!(g.release(800, &model, layout).edit.is_none());
    }
    #[test]
    fn changed_scene_and_cancellation_cannot_commit_but_keyboard_hide_is_allowed() {
        let (model, layout) = fixture();
        let source = Source::Launcher("app2.desktop".into());
        for which in 0..7 {
            let mut g = armed(&model, layout, source.clone());
            g.motion(center(layout, 0));
            let mut next = model.clone();
            match which {
                0 => next.view = View::Home,
                1 => next.system_menu_open = true,
                2 => next.launcher_query = "changed".into(),
                3 => next.favorites.applications.reverse(),
                4 => next.applications[2].unavailable = Some("Gone".into()),
                5 => next.editing_favorites = true,
                _ => next.notifications.banner = Some((1, 1)),
            }
            assert_eq!(g.release(800, &next, layout), Release::default());
        }
        let mut g = armed(&model, layout, source.clone());
        g.motion(center(layout, 0));
        assert_eq!(
            g.release(
                800,
                &model,
                Layout {
                    width: 720.0,
                    height: 360.0,
                    ..layout
                }
            ),
            Release::default()
        );
        let mut g = armed(&model, layout, source.clone());
        g.cancel();
        assert_eq!(g.release(800, &model, layout), Release::default());
        let mut g = armed(&model, layout, source);
        g.motion(center(layout, 0));
        assert!(
            g.release(
                800,
                &model,
                Layout {
                    notification_height: 0.0,
                    keyboard_height: 240.0,
                    ..layout
                }
            )
            .edit
            .is_some()
        );
    }
    #[test]
    fn retained_notification_cannot_cancel_a_compact_search_hold_hiding_its_keyboard() {
        for banner in [false, true] {
            let (mut model, layout) = fixture();
            model.launcher.search_open = true;
            model.shell_text_focus = true;
            model
                .notifications
                .items
                .push(crate::notifications::Notification {
                    id: 1,
                    source: 1,
                    revision: 1,
                    app_name: "Fixture".into(),
                    summary: "Retained".into(),
                    body: String::new(),
                    actions: vec![],
                    urgent: false,
                    live: true,
                    orphaned: false,
                });
            model.notifications.banner = banner.then_some((1, 1));
            let typing = Layout {
                width: 720.0,
                height: 360.0,
                keyboard_height: 180.0,
                ..layout
            }
            .with_notifications(&model);
            let source = Source::Launcher("app2.desktop".into());
            let mut gesture = armed(&model, typing, source.clone());
            model.apply(crate::Command::ShellTextFocus(false));
            let dragging = Layout {
                keyboard_height: 0.0,
                ..typing
            }
            .with_notifications(&model);
            assert_eq!(typing.notification_height, dragging.notification_height);
            gesture.reconcile(&model, dragging);
            gesture.motion(center(dragging, 0));
            assert!(gesture.release(800, &model, dragging).edit.is_some());
            let mut interrupted = armed(&model, dragging, source);
            interrupted.motion(center(dragging, 0));
            model.apply(crate::Command::NotificationsOpen(true));
            let dashboard = dragging.with_notifications(&model);
            assert!(dashboard.notification_height > 0.0);
            assert_eq!(
                interrupted.release(800, &model, dashboard),
                Release::default()
            );
        }
    }

    #[test]
    fn dashboard_allows_new_favorite_contacts_and_geometry_changes_cancel_them() {
        let (mut model, layout) = fixture();
        model.notifications_open = true;
        let layout = layout.with_notifications(&model);
        let source = Source::Launcher("app2.desktop".into());
        let mut gesture = armed(&model, layout, source.clone());
        gesture.motion(center(layout, 0));
        assert!(gesture.release(800, &model, layout).edit.is_some());
        for change in [false, true] {
            let mut gesture = armed(&model, layout, source.clone());
            gesture.motion(center(layout, 0));
            let mut next = model.clone();
            let mut next_layout = layout;
            if change {
                next.notifications_open = false;
            } else {
                next_layout.notification_height -= 1.0;
            }
            assert_eq!(gesture.release(800, &next, next_layout), Release::default());
        }
    }

    #[test]
    fn edge_dwell_previews_pages_and_only_a_valid_release_places_the_identity() {
        use crate::launcher::{Edit as PageEdit, PageId};
        let (mut model, layout) = fixture();
        model
            .launcher
            .settings
            .edit(PageEdit::Create("Work".into()), &model.applications)
            .unwrap();
        model
            .launcher
            .settings
            .edit(PageEdit::Select(PageId(1)), &model.applications)
            .unwrap();
        let before = model.launcher.settings.clone();
        let favorites = model.favorites.clone();
        let source = Source::Launcher("app2.desktop".into());
        let mut g = armed(&model, layout, source.clone());
        g.motion((350.0, 300.0));
        g.tick(800);
        g.tick(1399);
        assert_eq!(g.page(), Some(PageId(1)));
        g.tick(1400);
        assert_eq!(g.page(), Some(PageId(2)));
        assert_eq!(model.launcher.settings, before);
        g.motion((180.0, 300.0));
        g.page_target(Some((PageId(2), 0)));
        let release = g.release(1500, &model, layout);
        assert!(release.consumed && release.edit.is_none());
        let effects = model.apply(Command::LauncherAction(crate::launcher::Action::Edit(
            release.page_edit.unwrap(),
        )));
        assert!(matches!(effects.as_slice(), [Effect::Launcher(_)]));
        assert_eq!(
            model
                .launcher
                .settings
                .applications(PageId(2), &model.applications)[0]
                .id,
            "app2.desktop"
        );
        assert_eq!(model.favorites, favorites);
        assert_eq!(g.release(1501, &model, layout), Release::default());
        for change in 0..4 {
            let mut g = armed(&model, layout, source.clone());
            g.motion((180.0, 300.0));
            g.page_target(Some((PageId(2), 0)));
            let mut next = model.clone();
            let mut geometry = layout;
            match change {
                0 => next.launcher.menu_open = true,
                1 => {
                    next.launcher
                        .settings
                        .edit(PageEdit::Create("Changed".into()), &next.applications)
                        .unwrap();
                }
                2 => next.applications.retain(|a| a.id != "app2.desktop"),
                _ => geometry.width = 720.0,
            }
            assert_eq!(g.release(1600, &next, geometry), Release::default());
        }
    }
    #[test]
    fn unavailable_applications_can_be_arranged_but_cannot_become_favorites() {
        use crate::launcher::PageId;
        let (mut model, layout) = fixture();
        model.applications[2].unavailable = Some("Unavailable".into());
        let source = Source::Launcher("app2.desktop".into());
        let mut g = armed(&model, layout, source.clone());
        g.motion(center(layout, 0));
        assert_eq!(
            g.preview(&model, layout).unwrap().target,
            Some(Target::Unavailable(0))
        );
        let release = g.release(800, &model, layout);
        assert!(release.edit.is_none() && release.page_edit.is_none());
        let mut g = armed(&model, layout, source);
        g.motion((180.0, 300.0));
        g.page_target(Some((PageId(1), 0)));
        let release = g.release(800, &model, layout);
        assert!(release.edit.is_none() && release.page_edit.is_some());
    }
}
