use floem::event::EventListener;
use floem::peniko::Color;
use floem::reactive::{create_effect, RwSignal};
use floem::taffy::FlexDirection;
use floem::views::{dyn_stack, h_stack, text_input, v_stack, Decorators};
use floem::IntoView;
use rdaw_api::{Backend, TrackEvent, TrackId};
use rdaw_ui_kit::{button, ColorKind, Level};

use crate::api;

pub fn track_view<B: Backend>(id: TrackId, indent: usize) -> impl IntoView {
    let children = RwSignal::new(Vec::new());

    api::subscribe_track::<B>(id, move |event| {
        if let TrackEvent::ChildrenChanged { new_children } = event {
            children.set(new_children);
        }
    });

    let add_child = move || {
        api::create_track::<B>("Unnamed".into(), move |child_id| {
            api::insert_track_child::<B>(id, child_id, children.with(|v| v.len()));
        });
    };

    let draggable = RwSignal::new(None);

    v_stack((
        track_control_panel::<B>(id),
        dyn_stack(
            move || children.get(),
            move |id| *id,
            move |id| {
                let is_dragging = RwSignal::new(false);
                let is_dragging_over = RwSignal::new(false);

                track_view::<B>(id, indent + 1)
                    .keyboard_navigatable()
                    .draggable()
                    .style(move |s| {
                        s.apply_if(is_dragging.get(), |s| s.background(Color::RED))
                            .apply_if(is_dragging_over.get(), |s| {
                                s.border_top(4.0).border_color(Color::BLUE)
                            })
                    })
                    .dragging_style(|s| s.background(Color::TRANSPARENT).border_top(0.0))
                    .on_event_stop(EventListener::DragStart, move |_| {
                        is_dragging.set(true);
                        draggable.set(Some(id));
                    })
                    .on_event_stop(EventListener::DragEnter, move |_| {
                        is_dragging_over.set(true)
                    })
                    .on_event_stop(EventListener::DragLeave, move |_| {
                        is_dragging_over.set(false)
                    })
                    .on_event_stop(EventListener::Drop, move |_| {
                        dbg!(draggable.get());
                    })
                    .on_event_cont(EventListener::DragEnd, move |_| {
                        is_dragging.set(false);
                        draggable.set(None);
                    })
            },
        )
        .style(|s| s.flex_direction(FlexDirection::Column)),
        button(ColorKind::Surface, Level::Mid, || "Add child")
            .on_click_stop(move |_| add_child())
            .style(|s| s.width(100.0)),
    ))
    .style(move |s| s.padding_left(10.0 * (indent as f32)))
}

fn track_control_panel<B: Backend>(id: TrackId) -> impl IntoView {
    let name = RwSignal::new(String::new());
    let editor_name = RwSignal::new(String::new());

    api::get_track_name::<B>(id, move |new_name| name.set(new_name));

    api::subscribe_track::<B>(id, move |event| {
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

        api::set_track_name::<B>(id, editor_name.clone());

        editor_name
    });

    create_effect(move |_| {
        editor_name.set(name.get());
    });

    h_stack((text_input(editor_name).placeholder("Name"),)).style(|s| {
        s.padding(10)
            .border(1)
            .border_color(Color::BLACK)
            .background(Color::WHITE)
    })
}

fn track_timeline<B: Backend>(_id: TrackId) -> impl IntoView {
    "TODO"
}
