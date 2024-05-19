use floem::event::{Event, EventListener, EventPropagation};
use floem::peniko::Color;
use floem::reactive::{create_effect, RwSignal};
use floem::taffy::{Display, FlexDirection, Position};
use floem::views::{dyn_stack, empty, h_stack, text_input, v_stack, Decorators};
use floem::{IntoView, View};
use rdaw_api::{Backend, TrackEvent, TrackId};
use rdaw_core::collections::ImHashMap;
use rdaw_ui_kit::{button, ColorKind, Level};

use crate::api;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct Node {
    id: TrackId,
    parent: Option<TrackId>,
    index: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum DropLocation {
    Forbidden,
    Before(Node),
    After(Node),
    Inside(Node),
}

#[derive(Clone, Copy)]
struct State {
    show_add: bool,
    selection: RwSignal<Option<Node>>,
    is_dragging: RwSignal<bool>,
    drop_location: RwSignal<DropLocation>,
    parents: RwSignal<ImHashMap<TrackId, TrackId>>,
}

pub fn track_tree_view<B: Backend>(id: TrackId, show_add: bool) -> impl IntoView {
    let node = Node {
        id,
        parent: None,
        index: 0,
    };

    let state = State {
        show_add,
        selection: RwSignal::new(None),
        is_dragging: RwSignal::new(false),
        drop_location: RwSignal::new(DropLocation::Forbidden),
        parents: RwSignal::new(ImHashMap::default()),
    };

    tree_node::<B>(node, state)
}

fn tree_node<B: Backend>(node: Node, state: State) -> impl IntoView {
    let children = RwSignal::new(Vec::new());

    api::get_track_children::<B>(node.id, move |new_children| {
        children.set(new_children);
    });

    api::subscribe_track::<B>(node.id, move |event| {
        if let TrackEvent::ChildrenChanged { new_children } = event {
            state.parents.update(|parents| {
                for &child in &new_children {
                    parents.insert(child, node.id);
                }
            });

            children.set(new_children);
        }
    });

    let add_child = move || {
        api::create_track::<B>("Unnamed".into(), move |child_id| {
            api::insert_track_child::<B>(node.id, child_id, children.with(|v| v.len()))
        });
    };

    let add_child_button = button(ColorKind::Surface, Level::Mid, || "Add child")
        .on_click_stop(move |_| add_child())
        .style(move |s| s.width(100.0).apply_if(!state.show_add, |s| s.hide()));

    let child_views = dyn_stack(
        move || children.get().into_iter().enumerate(),
        move |v| *v,
        move |(index, id)| {
            tree_node::<B>(
                Node {
                    id,
                    index,
                    parent: Some(node.id),
                },
                state,
            )
        },
    )
    .style(|s| s.flex_direction(FlexDirection::Column).padding_left(20.0));

    let marker = move |location| {
        empty().style(move |s| {
            let is_visible = state.is_dragging.get() && state.drop_location.get() == location;
            let display = if is_visible {
                Display::Block
            } else {
                Display::None
            };

            let s = s
                .display(display)
                .position(Position::Absolute)
                .height(10)
                .inset_left(4.0)
                .inset_right(4.0)
                .border(2.0)
                .border_color(Color::BLUE);

            match location {
                DropLocation::Forbidden => s,
                DropLocation::Before { .. } => s.inset_top(-5.0),
                DropLocation::After { .. } => s.inset_bottom(-5.0),
                DropLocation::Inside { .. } => {
                    if children.with(|v| v.is_empty()) {
                        s.inset_bottom(-5.0).inset_left_pct(25.0)
                    } else {
                        s.inset_bottom(-5.0).inset_left(24.0)
                    }
                }
            }
        })
    };

    let before_marker = marker(DropLocation::Before(node));
    let inside_marker = marker(DropLocation::Inside(node));
    let after_marker = marker(DropLocation::After(node));

    let this_view = track_control_panel::<B>(node.id).into_view();
    let this_view_id = this_view.id();
    let this_view = this_view
        .style(move |s| {
            s.apply_if(state.selection.get() == Some(node), |s| {
                s.background(Color::rgba8(255, 0, 0, 20))
            })
        })
        .on_event(EventListener::PointerDown, move |ev| {
            let Event::PointerDown(ev) = ev else {
                return EventPropagation::Continue;
            };

            if !ev.button.is_primary() {
                return EventPropagation::Continue;
            }

            state.selection.set(Some(node));
            state.drop_location.set(DropLocation::Forbidden);
            state.is_dragging.set(true);

            EventPropagation::Stop
        })
        .on_event(EventListener::PointerMove, move |ev| {
            let Event::PointerMove(ev) = ev else {
                return EventPropagation::Continue;
            };

            if !state.is_dragging.get() {
                return EventPropagation::Continue;
            }

            let Some(sel_node) = state.selection.get() else {
                return EventPropagation::Continue;
            };

            if node == sel_node {
                state.drop_location.set(DropLocation::Forbidden);
                return EventPropagation::Continue;
            }

            let Some(size) = this_view_id.get_size() else {
                return EventPropagation::Continue;
            };

            let location = if ev.pos.y < size.height * 0.5 {
                if sel_node.parent == node.parent && sel_node.index + 1 == node.index {
                    DropLocation::Forbidden
                } else {
                    DropLocation::Before(node)
                }
            } else {
                if ev.pos.x > size.width * 0.75 || !children.get().is_empty() {
                    DropLocation::Inside(node)
                } else {
                    if sel_node.parent == node.parent && sel_node.index == node.index + 1 {
                        DropLocation::Forbidden
                    } else {
                        DropLocation::After(node)
                    }
                }
            };

            state.drop_location.set(location);

            EventPropagation::Stop
        });

    v_stack((
        before_marker,
        v_stack((this_view, inside_marker)).style(|s| s.position(Position::Relative)),
        child_views,
        after_marker,
        add_child_button,
    ))
    .debug_name("Track")
    .style(|s| s.position(Position::Relative))
    .on_event_stop(EventListener::PointerUp, move |ev| {
        let Event::PointerUp(ev) = ev else {
            return;
        };

        if !ev.button.is_primary() || !state.is_dragging.get() {
            return;
        }

        state.is_dragging.set(false);

        let Some(node) = state.selection.get() else {
            return;
        };

        let old_index = node.index;
        let Some(old_parent) = node.parent else {
            return;
        };

        let drop_location = state.drop_location.get();

        let (new_parent, new_index) = match drop_location {
            DropLocation::Before(Node {
                parent: Some(new_parent),
                index: before_index,
                ..
            }) => {
                if old_parent == new_parent && old_index < before_index {
                    (new_parent, before_index - 1)
                } else {
                    (new_parent, before_index)
                }
            }

            DropLocation::After(Node {
                parent: Some(new_parent),
                index: after_index,
                ..
            }) => {
                if old_parent == new_parent && old_index < after_index {
                    (new_parent, after_index)
                } else {
                    (new_parent, after_index + 1)
                }
            }

            DropLocation::Inside(Node { id: new_parent, .. }) => (new_parent, 0),

            _ => return,
        };

        api::move_track::<B>(old_parent, old_index, new_parent, new_index);
    })
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

    h_stack((text_input(editor_name).placeholder("Name"),))
        .style(|s| s.padding(10).border(1).border_color(Color::BLACK))
}
