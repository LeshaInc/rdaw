use floem::event::Event;
use floem::reactive::{create_effect, RwSignal};
use floem::views::{h_stack, text_input, Decorators};
use floem::IntoView;
use rdaw_api::track::{TrackEvent, TrackId};
use rdaw_ui_kit::{button, ColorKind, Level};

use crate::api;

pub fn track_control(id: TrackId) -> impl IntoView {
    let name = RwSignal::new(String::new());
    let editor_name = RwSignal::new(String::new());

    api::get_track_name(id, move |new_name| name.set(new_name));

    api::subscribe_track(id, move |event| {
        if let TrackEvent::NameChanged { new_name } = event {
            name.set(new_name)
        }
    });

    create_effect(move |old| {
        let editor_name = editor_name.get();
        let name = name.get();

        if old.is_none() || old.is_some_and(|v| v == editor_name) || editor_name == name {
            return editor_name;
        };

        api::set_track_name(id, editor_name.clone());

        editor_name
    });

    create_effect(move |_| {
        editor_name.set(name.get());
    });

    let add_child = move |_ev: &Event| {
        api::create_track(move |child_id| {
            api::append_track_child(id, child_id);
        });
    };

    let add_child_button = button(ColorKind::Surface, Level::Mid, || "Add child")
        .on_click_stop(add_child)
        .style(move |s| s.width(100.0));

    h_stack((
        text_input(editor_name).placeholder("Name"),
        add_child_button,
    ))
    .style(move |s| s.padding(10))
}
