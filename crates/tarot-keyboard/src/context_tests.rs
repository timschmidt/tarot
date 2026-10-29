use super::*;

fn frame(state: &mut Keyboard) -> Vec<ui::Action> {
    let events = tarot_ui::keyboard::take_pointer_frame(&mut state.events);
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(360.0, 240.0),
        )),
        events,
        focused: true,
        time: Some(state.context.cumulative_frame_nr() as f64 / 60.0),
        ..Default::default()
    };
    tarot_ui::theme::install(&state.context);
    let mut actions = Vec::new();
    let _ = state.context.run(input, |ctx| {
        actions = state.ui.show(ctx, false, false);
    });
    actions
}

#[test]
fn context_reset_cancels_held_and_queued_clicks_but_keeps_the_next_click() {
    for delivered_press in [false, true] {
        let (_server, client) = UnixStream::pair().unwrap();
        let connection = Connection::from_socket(client).unwrap();
        let queue = connection.new_event_queue();
        let mut state = Keyboard::new(connection, queue.handle(), false);
        for _ in 0..3 {
            assert!(frame(&mut state).is_empty());
        }
        state.position = egui::pos2(106.14286, 165.0);
        state.events.push(egui::Event::PointerMoved(state.position));
        state.button(true);
        if delivered_press {
            assert!(frame(&mut state).is_empty());
        } else {
            state.button(false);
        }
        state.contacts.insert(0);
        state.touch_owner = Some(0);
        state.frame_pending = true;
        let frame_number = state.context.cumulative_frame_nr();
        state.reset_context_input();
        assert!(state.contacts.contains(&0) && state.touch_cancelled);
        assert!(state.touch_owner.is_none() && state.frame_pending);
        assert_eq!(state.context.cumulative_frame_nr(), frame_number);
        while !state.events.is_empty() {
            assert!(frame(&mut state).is_empty());
        }
        // A late physical pointer release must not commit the old held key.
        state.button(false);
        assert!(frame(&mut state).is_empty());
        state.contacts.clear();
        state.touch_cancelled = false;
        state.events.push(egui::Event::PointerMoved(state.position));
        state.button(true);
        state.button(false);
        assert_eq!(frame(&mut state), vec![ui::Action::Text("x".into())]);
    }
}
