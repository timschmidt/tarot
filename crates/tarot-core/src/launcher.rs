//! Named pages own desktop identities; discovery and Quick Launch remain independent.
use crate::Application;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
mod paging;
pub use paging::{Swipe, SwipePreview};

pub const MAX_PAGES: usize = 32;
pub const MAX_ITEMS: usize = 512;
pub const EDGE_DWELL_MS: u64 = 600;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PageId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    pub id: PageId,
    pub name: String,
    pub applications: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub pages: Vec<Page>,
    pub selected: PageId,
    pub inbox: PageId,
    next_id: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            pages: vec![Page {
                id: PageId(1),
                name: "Applications".into(),
                applications: vec![],
            }],
            selected: PageId(1),
            inbox: PageId(1),
            next_id: 2,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edit {
    Select(PageId),
    Navigate(i32),
    Create(String),
    Rename {
        page: PageId,
        name: String,
    },
    Reorder {
        page: PageId,
        at: usize,
    },
    Remove(PageId),
    Place {
        application: String,
        page: PageId,
        at: usize,
    },
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.trim() == name
        && name.chars().count() <= 64
        && !name.chars().any(char::is_control)
}

impl Settings {
    pub fn valid(&self) -> bool {
        let mut pages = HashSet::new();
        let mut apps = HashSet::new();
        let mut bytes = 0;
        !self.pages.is_empty()
            && self.pages.len() <= MAX_PAGES
            && self.next_id <= i64::MAX as u64
            && self.pages.iter().all(|p| {
                bytes += p.name.len();
                p.id.0 > 0
                    && p.id.0 < self.next_id
                    && pages.insert(p.id)
                    && valid_name(&p.name)
                    && p.applications.iter().all(|id| {
                        bytes += id.len();
                        crate::quick_launch::valid_id(id) && apps.insert(id)
                    })
            })
            && apps.len() <= MAX_ITEMS
            && bytes <= 16 * 1024
            && pages.contains(&self.selected)
            && pages.contains(&self.inbox)
    }
    pub fn page(&self, id: PageId) -> Option<&Page> {
        self.pages.iter().find(|p| p.id == id)
    }
    pub fn neighbor(&self, id: PageId, delta: i32) -> Option<PageId> {
        let at = self.pages.iter().position(|p| p.id == id)? as i64 + i64::from(delta);
        usize::try_from(at)
            .ok()
            .and_then(|i| self.pages.get(i))
            .map(|p| p.id)
    }
    pub fn applications<'a>(
        &self,
        page: PageId,
        catalog: &'a [Application],
    ) -> Vec<&'a Application> {
        let Some(page) = self.page(page) else {
            return vec![];
        };
        let mut result: Vec<_> = page
            .applications
            .iter()
            .filter_map(|id| catalog.iter().find(|a| a.id == *id))
            .collect();
        if page.id == self.inbox {
            let assigned: HashSet<_> = self
                .pages
                .iter()
                .flat_map(|p| p.applications.iter())
                .collect();
            result.extend(catalog.iter().filter(|a| !assigned.contains(&a.id)));
        }
        result
    }
    pub fn edit(&mut self, edit: Edit, catalog: &[Application]) -> Result<bool, &'static str> {
        let mut next = self.clone();
        match edit {
            Edit::Select(id) => {
                if next.page(id).is_none() {
                    return Err("Page no longer exists");
                }
                next.selected = id;
            }
            Edit::Navigate(delta) => {
                if let Some(id) = next.neighbor(next.selected, delta) {
                    next.selected = id
                }
            }
            Edit::Create(name) => {
                let name = name.trim().to_owned();
                if !valid_name(&name) {
                    return Err("Use a page name of 1–64 characters");
                }
                let id = PageId(next.next_id);
                next.next_id = next
                    .next_id
                    .checked_add(1)
                    .ok_or("Page identity limit reached")?;
                next.pages.push(Page {
                    id,
                    name,
                    applications: vec![],
                });
                next.selected = id;
            }
            Edit::Rename { page, name } => {
                let name = name.trim().to_owned();
                if !valid_name(&name) {
                    return Err("Use a page name of 1–64 characters");
                }
                next.pages
                    .iter_mut()
                    .find(|p| p.id == page)
                    .ok_or("Page no longer exists")?
                    .name = name;
            }
            Edit::Reorder { page, at } => {
                if at >= next.pages.len() {
                    return Err("Page position no longer exists");
                }
                let from = next
                    .pages
                    .iter()
                    .position(|p| p.id == page)
                    .ok_or("Page no longer exists")?;
                let page = next.pages.remove(from);
                next.pages.insert(at, page);
            }
            Edit::Remove(id) => {
                if next.pages.len() == 1 {
                    return Err("Keep at least one Launcher page");
                }
                let at = next
                    .pages
                    .iter()
                    .position(|p| p.id == id)
                    .ok_or("Page no longer exists")?;
                let removed = next.pages.remove(at);
                if next.inbox == id {
                    next.inbox = next.pages[0].id
                }
                if next.selected == id {
                    next.selected = next.pages[at.min(next.pages.len() - 1)].id
                }
                next.pages
                    .iter_mut()
                    .find(|p| p.id == next.inbox)
                    .unwrap()
                    .applications
                    .extend(removed.applications);
            }
            Edit::Place {
                application,
                page,
                at,
            } => {
                if !catalog.iter().any(|a| a.id == application) {
                    return Err("Application no longer exists");
                }
                if next.page(page).is_none() {
                    return Err("Page no longer exists");
                }
                let mut visible: Vec<_> = next
                    .applications(page, catalog)
                    .iter()
                    .map(|a| a.id.clone())
                    .collect();
                visible.retain(|id| *id != application);
                visible.insert(at.min(visible.len()), application.clone());
                for p in &mut next.pages {
                    p.applications.retain(|id| *id != application)
                }
                let target = next.pages.iter_mut().find(|p| p.id == page).unwrap();
                // Keep absent entries in their slots so an app upgrade or
                // temporary removal does not erase the user's arrangement.
                let mut visible = visible.into_iter();
                let mut merged = Vec::new();
                for id in &target.applications {
                    if catalog.iter().any(|a| a.id == *id) {
                        if let Some(id) = visible.next() {
                            merged.push(id)
                        }
                    } else {
                        merged.push(id.clone())
                    }
                }
                merged.extend(visible);
                target.applications = merged;
                next.selected = page;
            }
        }
        if !next.valid() {
            return Err("Launcher arrangement limit reached");
        }
        let changed = next != *self;
        *self = next;
        Ok(changed)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rename {
    pub page: Option<PageId>,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Edit(Edit),
    Menu(bool),
    Search(bool),
    BeginName(Option<PageId>),
    Name(String),
    AcceptName,
    Dismiss,
}

#[derive(Clone, Debug, Default)]
pub struct State {
    pub settings: Settings,
    pub menu_open: bool,
    pub search_open: bool,
    pub rename: Option<Rename>,
    pub notice: Option<String>,
    pub available: bool,
    pub saved: bool,
    pub save_error: Option<String>,
    pub name_epoch: u64,
}

impl State {
    pub fn dismiss(&mut self) {
        self.menu_open = false;
        self.search_open = false;
        self.rename = None;
        self.notice = None;
    }
    pub fn action(
        &mut self,
        action: Action,
        catalog: &[Application],
        query: &mut String,
        focus: &mut bool,
    ) -> bool {
        let keep_menu = self.menu_open
            && matches!(action, Action::Edit(Edit::Reorder { .. } | Edit::Remove(_)));
        let edit = match action {
            Action::Edit(edit) => Some(edit),
            Action::Menu(open) => {
                self.menu_open = open;
                *focus = false;
                None
            }
            Action::Search(open) => {
                self.menu_open = false;
                self.rename = None;
                self.search_open = open;
                *focus = open;
                if !open {
                    query.clear()
                }
                None
            }
            Action::BeginName(page) => {
                if page.is_none() || page.is_some_and(|p| self.settings.page(p).is_some()) {
                    self.name_epoch = self.name_epoch.wrapping_add(1);
                    let text = page
                        .and_then(|p| self.settings.page(p))
                        .map_or(String::new(), |p| p.name.clone());
                    self.rename = Some(Rename { page, text });
                    self.menu_open = false;
                    self.search_open = false;
                    self.notice = None;
                    query.clear();
                    *focus = true;
                }
                None
            }
            Action::Name(text) => {
                if text.len() <= 1024
                    && let Some(r) = &mut self.rename
                {
                    r.text = text;
                    self.notice = None;
                }
                None
            }
            Action::AcceptName => self.rename.as_ref().map(|r| match r.page {
                Some(page) => Edit::Rename {
                    page,
                    name: r.text.clone(),
                },
                None => Edit::Create(r.text.clone()),
            }),
            Action::Dismiss => {
                self.dismiss();
                query.clear();
                *focus = false;
                None
            }
        };
        let Some(edit) = edit else { return false };
        match self.settings.edit(edit, catalog) {
            Ok(changed) => {
                self.dismiss();
                self.menu_open = keep_menu;
                query.clear();
                *focus = false;
                changed
            }
            Err(error) => {
                self.notice = Some(error.into());
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn catalog() -> Vec<Application> {
        ["a", "b", "c"]
            .iter()
            .map(|id| Application {
                id: format!("{id}.desktop"),
                name: (*id).into(),
                description: String::new(),
                icon: None,
                keywords: vec![],
                unavailable: None,
            })
            .collect()
    }
    #[test]
    fn identities_survive_page_moves_discovery_removal_and_reinstallation() {
        let apps = catalog();
        let mut s = Settings::default();
        let inbox = s.inbox;
        s.edit(Edit::Create("Work".into()), &apps).unwrap();
        let work = s.selected;
        s.edit(
            Edit::Place {
                application: "b.desktop".into(),
                page: work,
                at: 0,
            },
            &apps,
        )
        .unwrap();
        s.edit(
            Edit::Place {
                application: "c.desktop".into(),
                page: inbox,
                at: 0,
            },
            &apps,
        )
        .unwrap();
        assert_eq!(
            s.applications(inbox, &apps)
                .iter()
                .map(|a| a.id.as_str())
                .collect::<Vec<_>>(),
            ["c.desktop", "a.desktop"]
        );
        s.edit(Edit::Reorder { page: work, at: 0 }, &apps).unwrap();
        assert_eq!(s.inbox, inbox);
        s.edit(Edit::Remove(work), &apps).unwrap();
        assert_eq!(s.selected, inbox);
        let without = apps[0..2].to_vec();
        assert_eq!(s.applications(inbox, &without).len(), 2);
        assert_eq!(
            s.applications(inbox, &apps)
                .iter()
                .map(|a| a.id.as_str())
                .collect::<Vec<_>>(),
            ["c.desktop", "a.desktop", "b.desktop"]
        );
        assert!(s.valid());
    }
    #[test]
    fn edits_are_atomic_bounded_and_cannot_delete_the_last_page() {
        let apps = catalog();
        let mut s = Settings::default();
        let original = s.clone();
        for edit in [
            Edit::Remove(s.selected),
            Edit::Create("\n".into()),
            Edit::Place {
                application: "absent.desktop".into(),
                page: s.selected,
                at: 0,
            },
        ] {
            assert!(s.edit(edit, &apps).is_err());
            assert_eq!(s, original);
        }
        for n in 1..MAX_PAGES {
            s.edit(Edit::Create(format!("Page {n}")), &apps).unwrap();
        }
        let before = s.clone();
        assert!(s.edit(Edit::Create("Too many".into()), &apps).is_err());
        assert_eq!(s, before);
        let mut duplicate = Settings::default();
        duplicate.pages[0].applications = vec!["a.desktop".into(); 2];
        assert!(!duplicate.valid());
    }
    #[test]
    fn name_drafts_cancel_without_persistence_and_valid_acceptance_selects_new_page() {
        let mut state = State::default();
        let mut query = "old".into();
        let mut focus = false;
        let apps = catalog();
        assert!(!state.action(Action::BeginName(None), &apps, &mut query, &mut focus));
        assert!(focus && query.is_empty());
        state.action(Action::Name("Work".into()), &apps, &mut query, &mut focus);
        state.action(Action::Dismiss, &apps, &mut query, &mut focus);
        assert_eq!(state.settings, Settings::default());
        state.action(Action::BeginName(None), &apps, &mut query, &mut focus);
        assert!(!state.action(Action::AcceptName, &apps, &mut query, &mut focus));
        assert!(state.rename.is_some() && state.notice.is_some());
        state.action(Action::Name(" Work ".into()), &apps, &mut query, &mut focus);
        assert!(state.action(Action::AcceptName, &apps, &mut query, &mut focus));
        assert_eq!(
            state.settings.page(state.settings.selected).unwrap().name,
            "Work"
        );
        assert!(!focus && state.rename.is_none());
    }
}
