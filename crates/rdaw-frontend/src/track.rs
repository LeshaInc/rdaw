use floem::event::{Event, EventListener, EventPropagation};
use floem::peniko::Color;
use floem::reactive::{batch, create_effect, create_memo, RwSignal};
use floem::taffy::{Display, FlexDirection, Position};
use floem::views::{dyn_stack, empty, h_stack, label, scroll, text_input, v_stack, Decorators};
use floem::{IntoView, View};
use rdaw_api::{Backend, TrackEvent, TrackId};
use rdaw_core::collections::{HashSet, ImHashMap, ImVec};
use rdaw_ui_kit::{button, ColorKind, Level};

use crate::api;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
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
    root: Node,
    selection: RwSignal<Option<Node>>,
    transitive_selection: RwSignal<HashSet<Node>>,
    is_dragging: RwSignal<bool>,
    drop_location: RwSignal<DropLocation>,
    children: RwSignal<ImHashMap<Node, ImVec<Node>>>,
}

pub fn track_tree_view<B: Backend>(id: TrackId) -> impl IntoView {
    let root = Node {
        id,
        parent: None,
        index: 0,
    };

    let state = State {
        root,
        selection: RwSignal::new(None),
        transitive_selection: RwSignal::new(HashSet::default()),
        is_dragging: RwSignal::new(false),
        drop_location: RwSignal::new(DropLocation::Forbidden),
        children: RwSignal::new(ImHashMap::default()),
    };

    scroll(h_stack((
        tcp_tree_node::<B>(root, state).style(|s| s.width(400.0)),
        tap_tree_node::<B>(root, state),
    )))
}

fn tcp_tree_node<B: Backend>(node: Node, state: State) -> impl IntoView {
    let tcp_view = track_control_panel::<B>(node.id).into_view();
    let tcp_view_id = tcp_view.id();

    let children = create_memo(move |_| {
        state
            .children
            .with(|c| c.get(&node).cloned().unwrap_or_default())
    });

    let has_children = create_memo(move |_| !children.get().is_empty());

    let on_drag = move |ev: &Event| {
        let Event::PointerDown(ev) = ev else {
            return EventPropagation::Continue;
        };

        if !ev.button.is_primary() {
            return EventPropagation::Continue;
        }

        batch(move || {
            state.selection.set(Some(node));
            state.drop_location.set(DropLocation::Forbidden);
            state.is_dragging.set(true);
            state.transitive_selection.update(|selection| {
                let children = state.children.get_untracked();

                let mut stack = Vec::with_capacity(64);
                stack.push(node);

                selection.clear();

                while let Some(node) = stack.pop() {
                    selection.insert(node);

                    let Some(children) = children.get(&node) else {
                        continue;
                    };

                    for &child in children {
                        stack.push(child);
                    }
                }
            });
        });

        EventPropagation::Stop
    };

    let while_dragging = move |ev: &Event| {
        let Event::PointerMove(ev) = ev else {
            return EventPropagation::Continue;
        };

        if !state.is_dragging.get() {
            return EventPropagation::Continue;
        }

        let Some(sel_node) = state.selection.get() else {
            return EventPropagation::Continue;
        };

        let Some(size) = tcp_view_id.get_size() else {
            return EventPropagation::Continue;
        };

        if state
            .transitive_selection
            .with_untracked(|v| v.contains(&node))
        {
            if state.drop_location.get_untracked() != DropLocation::Forbidden {
                state.drop_location.set(DropLocation::Forbidden);
            }
            return EventPropagation::Stop;
        }

        let location = if node == state.root {
            DropLocation::Inside(node)
        } else if ev.pos.y < size.height * 0.5 {
            if sel_node.parent == node.parent && sel_node.index + 1 == node.index {
                DropLocation::Forbidden
            } else {
                DropLocation::Before(node)
            }
        } else if ev.pos.x > size.width * 0.75 || has_children.get() {
            DropLocation::Inside(node)
        } else if sel_node.parent == node.parent && sel_node.index == node.index + 1 {
            DropLocation::Forbidden
        } else {
            DropLocation::After(node)
        };

        let old_location = state.drop_location.get_untracked();
        if location != old_location {
            state.drop_location.set(location);
        }

        EventPropagation::Stop
    };

    let on_drop = move |ev: &Event| {
        let Event::PointerUp(ev) = ev else {
            return;
        };

        if !ev.button.is_primary() || !state.is_dragging.get() {
            return;
        }

        state.is_dragging.set(false);

        let Some(node) = state.selection.get_untracked() else {
            return;
        };

        let old_index = node.index;
        let Some(old_parent) = node.parent else {
            return;
        };

        let drop_location = state.drop_location.get_untracked();

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
    };

    let child_views = dyn_stack(
        move || children.get(),
        move |v| *v,
        move |node| tcp_tree_node::<B>(node, state),
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
                    if has_children.get() {
                        s.inset_bottom(-5.0).inset_left(24.0)
                    } else {
                        s.inset_bottom(-5.0).inset_left_pct(25.0)
                    }
                }
            }
        })
    };

    let before_marker = marker(DropLocation::Before(node));
    let inside_marker = marker(DropLocation::Inside(node));
    let after_marker = marker(DropLocation::After(node));

    let tcp_view = tcp_view
        .keyboard_navigatable()
        .style(move |s| {
            s.apply_if(state.selection.get() == Some(node), |s| {
                s.background(Color::rgba8(255, 0, 0, 20))
            })
        })
        .on_event(EventListener::PointerDown, on_drag)
        .on_event(EventListener::PointerMove, while_dragging);

    v_stack((
        before_marker,
        v_stack((tcp_view, inside_marker)).style(|s| s.position(Position::Relative)),
        child_views,
        after_marker,
    ))
    .debug_name("TcpTrackNode")
    .style(|s| s.position(Position::Relative))
    .on_event_stop(EventListener::PointerUp, on_drop)
}

fn tap_tree_node<B: Backend>(node: Node, state: State) -> impl IntoView {
    let tcp_view = track_arrangement_view::<B>(node.id).into_view();

    let set_children = move |new_children: ImVec<TrackId>| {
        state.children.update(move |children| {
            let new_children = new_children
                .into_iter()
                .enumerate()
                .map(|(index, id)| Node {
                    id,
                    parent: Some(node.id),
                    index,
                })
                .collect();
            children.insert(node, new_children);
        });
    };

    let children = create_memo(move |_| {
        state
            .children
            .with(|c| c.get(&node).cloned().unwrap_or_default())
    });

    api::get_track_children::<B>(node.id, move |new_children| {
        set_children(new_children);
    });

    api::subscribe_track::<B>(node.id, move |event| {
        if let TrackEvent::ChildrenChanged { new_children } = event {
            set_children(new_children);
        }
    });

    let child_views = dyn_stack(
        move || children.get(),
        move |v| *v,
        move |node| tap_tree_node::<B>(node, state),
    )
    .style(|s| s.flex_direction(FlexDirection::Column));

    v_stack((tcp_view, child_views))
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

    let add_child = move |_ev: &Event| {
        api::create_track::<B>(move |child_id| {
            api::append_track_child::<B>(id, child_id);
        });
    };

    let add_child_button = button(ColorKind::Surface, Level::Mid, || "Add child")
        .on_click_stop(add_child)
        .style(move |s| s.width(100.0));

    h_stack((
        text_input(editor_name).placeholder("Name"),
        add_child_button,
    ))
    .style(|s| {
        s.height(60.0)
            .padding(10)
            .border(1)
            .border_color(Color::BLACK)
    })
}

fn track_arrangement_view<B: Backend>(_id: TrackId) -> impl IntoView {
    label(move || "Arrangement view...").style(|s| s.height(60.0))
}
