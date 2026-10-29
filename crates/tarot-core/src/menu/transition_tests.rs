use super::*;

fn at(model: &mut ShellModel, millis: u64) {
    model.advance_menus(Duration::from_millis(millis));
}
fn model() -> ShellModel {
    let mut model = ShellModel::default();
    at(&mut model, 0);
    model.add_window(WindowId(1), "Editor".into(), "editor".into());
    model
}
fn choose(model: &mut ShellModel, target: Target) -> Vec<Effect> {
    model.apply(Command::Menu(Action::Choose {
        revision: model.menus.revision,
        target,
    }))
}

#[test]
fn elapsed_time_and_presented_retirement_control_native_focus() {
    let mut model = model();
    assert_eq!(
        model.apply(Command::Menu(Action::ToggleActivity)),
        vec![Effect::Focus(None)]
    );
    assert_eq!(model.menu_visual().opacity, 0.0);
    at(&mut model, 80);
    let middle = model.menu_visual();
    assert!(middle.opacity > 0.0 && middle.opacity < 1.0 && middle.offset_y < 0.0);
    at(&mut model, 10);
    assert_eq!(
        model.menu_visual(),
        middle,
        "an old timestamp cannot rewind a transition"
    );
    for _ in 0..20 {
        at(&mut model, 80);
        assert_eq!(model.menu_visual(), middle);
    }
    at(&mut model, 160);
    assert_eq!(model.menu_visual().opacity, 1.0);
    assert!(model.apply(Command::Menu(Action::Dismiss)).is_empty());
    at(&mut model, 220);
    assert!(model.menu_visual().closing && model.menu_visual().moving);
    let revision = model.menus.revision;
    assert!(model.present_menus(revision).is_empty());
    at(&mut model, 5000);
    assert_eq!(model.menu_retirement_token(), Some(revision));
    assert_eq!(model.menu_visual().opacity, 0.0);
    assert!(
        model.system_menu_open,
        "elapsed time alone cannot expose the underlying client"
    );
    assert!(model.present_menus(revision.wrapping_sub(1)).is_empty());
    assert_eq!(
        model.present_menus(revision),
        vec![Effect::Focus(Some(WindowId(1)))]
    );
    assert!(!model.system_menu_open);
    assert!(model.present_menus(revision).is_empty());
}

#[test]
fn a_close_choice_commits_once_after_its_frame_and_is_cancelled_by_owner_death() {
    for death in [false, true] {
        let mut model = model();
        model.add_window(WindowId(2), "Second".into(), "second".into());
        model.apply(Command::Menu(Action::ToggleActivity));
        at(&mut model, 160);
        assert!(choose(&mut model, Target::CloseCard(WindowId(2))).is_empty());
        assert!(!model.cards.iter().any(|c| c.closing));
        let revision = model.menus.revision;
        if death {
            model.remove_window(WindowId(2));
        }
        at(&mut model, 280);
        let effects = model.present_menus(revision);
        assert_eq!(effects.contains(&Effect::RequestClose(WindowId(2))), !death);
        assert!(!model.cards[0].closing);
        assert!(model.present_menus(revision).is_empty());
    }
}

#[test]
fn switching_or_reversing_dismissal_discards_the_old_choice_and_old_frame() {
    let mut model = model();
    model.apply(Command::Menu(Action::ToggleActivity));
    at(&mut model, 160);
    choose(&mut model, Target::CloseCard(WindowId(1)));
    let old = model.menus.revision;
    at(&mut model, 200);
    let pose = model.menu_visual();
    model.apply(Command::Menu(Action::ToggleDevice));
    assert_eq!(model.menu_visual().opacity, pose.opacity);
    assert_eq!(model.menu_visual().scrim, pose.scrim);
    at(&mut model, 500);
    assert!(model.present_menus(old).is_empty());
    assert_eq!(model.system_page, SystemPage::Device);
    model.apply(Command::Menu(Action::Dismiss));
    at(&mut model, 620);
    let token = model.menu_retirement_token().unwrap();
    assert!(
        !model
            .present_menus(token)
            .iter()
            .any(|e| matches!(e, Effect::RequestClose(_)))
    );
    assert!(!model.cards[0].closing);
}

#[test]
fn geometry_and_invalidated_capabilities_cannot_commit_a_deferred_action() {
    let mut model = model();
    model.apply(Command::Menu(Action::ToggleActivity));
    at(&mut model, 160);
    choose(&mut model, Target::CloseCard(WindowId(1)));
    let old = model.menus.revision;
    assert!(
        !model
            .reconfigure_menus(Duration::from_millis(900))
            .iter()
            .any(|e| matches!(e, Effect::RequestClose(_)))
    );
    assert!(model.present_menus(old).is_empty());
    assert!(!model.cards[0].closing);

    model.apply(Command::Launcher);
    model.apply(Command::Menu(Action::ToggleActivity));
    at(&mut model, 1100);
    choose(&mut model, Target::NewPage);
    for index in 1..launcher::MAX_PAGES {
        model
            .launcher
            .settings
            .edit(launcher::Edit::Create(format!("Page {index}")), &[])
            .unwrap();
    }
    at(&mut model, 1300);
    let token = model.menu_retirement_token().unwrap();
    model.present_menus(token);
    assert!(model.launcher.rename.is_none());
    assert!(!model.system_menu_open);
}

#[test]
fn reduced_motion_fades_without_translation_and_lock_never_waits() {
    let mut model = model();
    model.reduced_motion = true;
    at(&mut model, 0);
    model.lock_available = true;
    model.apply(Command::Menu(Action::ToggleDevice));
    at(&mut model, 50);
    assert_eq!(model.menu_visual().offset_y, 0.0);
    assert!(model.menu_visual().opacity > 0.0 && model.menu_visual().opacity < 1.0);
    at(&mut model, 100);
    assert_eq!(model.menu_visual().opacity, 1.0);
    assert!(choose(&mut model, Target::Lock).contains(&Effect::Lock));
    assert!(!model.system_menu_open && model.menu_retirement_token().is_none());
    model.apply(Command::Menu(Action::ToggleDevice));
    at(&mut model, 200);
    model.apply(Command::Menu(Action::Dismiss));
    at(&mut model, 299);
    assert!(model.menu_visual().moving && model.system_menu_open);
    at(&mut model, 300);
    assert!(model.menu_retirement_token().is_some());
}

#[test]
fn an_idle_clock_starts_a_fresh_animation_and_page_edits_keep_one_scrim() {
    let mut model = model();
    at(&mut model, 3_600_000);
    model.apply(Command::Launcher);
    model.apply(Command::LauncherAction(launcher::Action::Edit(
        launcher::Edit::Create("Work".into()),
    )));
    let work = model.launcher.settings.selected;
    model.apply(Command::Menu(Action::ToggleActivity));
    assert_eq!(model.menu_visual().opacity, 0.0);
    at(&mut model, 3_600_160);
    choose(&mut model, Target::Pages);
    choose(&mut model, Target::PageActions(work));
    assert!(
        choose(&mut model, Target::MovePage { page: work, at: 0 })
            .iter()
            .any(|e| matches!(e, Effect::Launcher(_)))
    );
    assert_eq!(model.menu_visual().opacity, 1.0);
    assert_eq!(model.menu_visual().scrim, 1.0);
    assert!(model.system_menu_open && model.launcher.menu_open);
    choose(&mut model, Target::RemovePage(work));
    assert_eq!(model.menu_visual().opacity, 1.0);
    assert_eq!(model.menus.panel, Panel::Pages);
}
