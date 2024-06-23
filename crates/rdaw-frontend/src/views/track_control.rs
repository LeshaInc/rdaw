use floem::event::Event;
use floem::reactive::{create_effect, RwSignal};
use floem::views::{h_stack, text_input, Decorators};
use floem::IntoView;
use rdaw_api::track::TrackId;
use rdaw_ui::task::stream_for_each;
use rdaw_ui::theme::{ColorKind, Level};
use rdaw_ui::views::button;

use crate::{api, get_document_id};

pub fn track_control(id: TrackId) -> impl IntoView {
    let document_id = get_document_id();
    let name = RwSignal::new(String::new());
    let editor_name = RwSignal::new(String::new());

    api::call(
        move |api| async move {
            let name = api.get_track_name(id).await?;
            let stream = api.subscribe_track_name(id).await?;
            Ok((name, stream))
        },
        move |(new_name, stream)| {
            name.set(new_name);

            stream_for_each(stream, move |new_name| name.set(new_name))
        },
    );

    create_effect(move |old| {
        let editor_name = editor_name.get();
        let name = name.get();

        if old.is_none() || old.is_some_and(|v| v == editor_name) || editor_name == name {
            return editor_name;
        };

        let name = editor_name.clone();
        api::call(
            move |api| async move { api.set_track_name(id, name).await },
            drop,
        );

        editor_name
    });

    create_effect(move |_| {
        editor_name.set(name.get());
    });

    let add_child = move |_ev: &Event| {
        api::call(
            move |api| async move {
                let child_id = api.create_track(document_id).await?;
                api.append_track_child(id, child_id).await
            },
            drop,
        );
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
