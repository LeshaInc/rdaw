use floem::event::{Event, EventListener, EventPropagation};
use floem::peniko::Color;
use floem::reactive::{batch, create_effect, create_memo, RwSignal};
use floem::style::CursorStyle;
use floem::taffy::{Display, Position};
use floem::views::{
    empty, h_stack, label, scroll, text_input, v_stack, virtual_stack, Decorators,
    VirtualDirection, VirtualItemSize, VirtualVector,
};
use floem::{IntoView, View};
use rdaw_api::{Backend, TrackEvent, TrackHierarchy, TrackHierarchyEvent, TrackId, TrackNode};
use rdaw_core::collections::{HashMap, HashSet, ImVec};
use rdaw_ui_kit::{button, ColorKind, Level};

use crate::api;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum DropLocation {
    Forbidden,
    Before(TrackNode),
    After(TrackNode),
    Inside(TrackNode),
}

#[derive(Clone, Copy)]
struct State {
    hierarchy: RwSignal<TrackHierarchy>,
    selection: RwSignal<Option<TrackNode>>,
    transitive_selection: RwSignal<HashSet<TrackId>>,
    is_dragging: RwSignal<bool>,
    drop_location: RwSignal<DropLocation>,
    min_track_height: f64,
    track_heights: RwSignal<HashMap<TrackNode, RwSignal<f64>>>,
}

pub fn track_tree_view<B: Backend>(root: TrackId) -> impl IntoView {
    let state = State {
        selection: RwSignal::new(None),
        transitive_selection: RwSignal::new(HashSet::default()),
        is_dragging: RwSignal::new(false),
        drop_location: RwSignal::new(DropLocation::Forbidden),
        hierarchy: RwSignal::new(TrackHierarchy::new(root)),
        min_track_height: 50.0,
        track_heights: RwSignal::new(HashMap::default()),
    };

    api::get_track_hierarchy::<B>(root, move |new_hierarchy| {
        state.hierarchy.set(new_hierarchy);
    });

    api::subscribe_track_hierarchy::<B>(root, move |event| {
        let TrackHierarchyEvent::ChildrenChanged { id, new_children } = event;
        state.hierarchy.update(|v| {
            v.set_children(id, new_children.into_iter().collect());
        });
    });

    let order = create_memo(move |_| {
        let mut order = ImVec::new();

        state.hierarchy.with(|hierarchy| {
            hierarchy.dfs(root, |node| {
                order.push_back(node);
            })
        });

        order
    });

    let get_height = move |node: &TrackNode| {
        state.track_heights.with(|heights| {
            heights
                .get(&node)
                .map(|signal| signal.get())
                .unwrap_or(state.min_track_height)
        })
    };

    let tcp_tree = virtual_stack(
        VirtualDirection::Vertical,
        VirtualItemSize::Fn(Box::new(get_height)),
        move || order.get(),
        move |node| *node,
        move |node| tcp_tree_node::<B>(state, node),
    );

    let tav_tree = virtual_stack(
        VirtualDirection::Vertical,
        VirtualItemSize::Fn(Box::new(move |(_, node)| get_height(node))),
        move || order.get().enumerate(),
        move |(idx, node)| (*node, idx % 2 == 0),
        move |(idx, node)| tav_tree_node::<B>(state, node, idx % 2 == 0),
    );

    scroll(
        h_stack((
            tcp_tree.style(|s| s.width(400.0)),
            tav_tree.style(|s| s.flex_grow(1.0)),
        ))
        .style(|s| s.width_full()),
    )
}

fn tcp_tree_node<B: Backend>(state: State, node: TrackNode) -> impl IntoView {
    let is_resizing = RwSignal::new(false);
    let prev_resizing_y = RwSignal::new(None);

    let track_height = create_memo(move |_| {
        state.track_heights.with(|v| {
            v.get(&node)
                .map(|v| v.get())
                .unwrap_or(state.min_track_height)
        })
    });

    let set_track_height = move |new_height| {
        if state.track_heights.with(|v| v.contains_key(&node)) {
            state.track_heights.with(|v| v[&node].set(new_height));
        } else {
            state.track_heights.update(|v| {
                v.insert(node, RwSignal::new(new_height));
            });
        }
    };

    let tcp_view = track_control_panel::<B>(node.id).into_view();
    let tcp_view_id = tcp_view.id();

    let track_resizer = empty();
    let track_resizer_id = track_resizer.id();

    let has_children = create_memo(move |_| {
        state
            .hierarchy
            .with(|h| h.children(node.id).next().is_some())
    });

    let select = move |ev: &Event| {
        let Event::PointerDown(ev) = ev else {
            return EventPropagation::Continue;
        };

        if !ev.button.is_primary() {
            return EventPropagation::Continue;
        }

        batch(move || {
            state.selection.set(Some(node));
            state.transitive_selection.update(|selection| {
                selection.clear();

                state.hierarchy.with(|hierarchy| {
                    hierarchy.dfs(node.id, |node| {
                        selection.insert(node.id);
                    });
                });
            });
        });

        EventPropagation::Stop
    };

    let resize_start = move |ev: &Event| {
        let Event::PointerMove(ev) = ev else {
            return EventPropagation::Continue;
        };

        let Some(layout) = track_resizer_id.get_layout() else {
            return EventPropagation::Continue;
        };

        batch(move || {
            is_resizing.set(true);
            prev_resizing_y.set(Some(layout.location.y as f64 + ev.pos.y));
        });

        EventPropagation::Stop
    };

    let resize_move = move |ev: &Event| {
        let Event::PointerMove(ev) = ev else {
            return EventPropagation::Continue;
        };

        if !is_resizing.get_untracked() {
            return EventPropagation::Continue;
        }

        let Some(layout) = track_resizer_id.get_layout() else {
            return EventPropagation::Continue;
        };

        let Some(prev_y) = prev_resizing_y.get_untracked() else {
            return EventPropagation::Continue;
        };

        batch(move || {
            let height = track_height.get_untracked();
            let delta = ev.pos.y + layout.location.y as f64 - prev_y;
            let new_height = (height + delta).max(state.min_track_height);
            let actual_delta = new_height - height;
            set_track_height(new_height);
            prev_resizing_y.set(Some(prev_y + actual_delta));
        });

        EventPropagation::Stop
    };

    let resize_end = move |_: &Event| {
        is_resizing.set(false);
    };

    let drag_start = move |_: &Event| {
        batch(move || {
            state.is_dragging.set(true);
            state.drop_location.set(DropLocation::Forbidden);
        });
    };

    let drag_over = move |ev: &Event| {
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
            .with_untracked(|v| v.contains(&node.id))
        {
            if state.drop_location.get_untracked() != DropLocation::Forbidden {
                state.drop_location.set(DropLocation::Forbidden);
            }
            return EventPropagation::Stop;
        }

        let location = if node.level == 0 {
            DropLocation::Inside(node)
        } else if ev.pos.y < size.height * 0.5 {
            if sel_node.parent == node.parent && sel_node.index + 1 == node.index {
                DropLocation::Forbidden
            } else {
                DropLocation::Before(node)
            }
        } else if ev.pos.x > size.width - 25.0 || has_children.get() {
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

    let drop_end = move |ev: &Event| {
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
            DropLocation::Before(TrackNode {
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

            DropLocation::After(TrackNode {
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

            DropLocation::Inside(TrackNode { id: new_parent, .. }) => (new_parent, 0),

            _ => return,
        };

        api::move_track::<B>(old_parent, old_index, new_parent, new_index);
    };

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
                DropLocation::Before { .. } => s.inset_top(-5.5),
                DropLocation::After { .. } => s.inset_bottom(-4.5),
                DropLocation::Inside { .. } => {
                    if has_children.get() {
                        s.inset_bottom(-5.5).inset_left(24.0)
                    } else {
                        s.inset_bottom(-5.5).inset_left_pct(25.0)
                    }
                }
            }
        })
    };

    let before_marker = marker(DropLocation::Before(node));
    let inside_marker = marker(DropLocation::Inside(node));
    let after_marker = marker(DropLocation::After(node));

    let track_selector = empty()
        .style(move |s| {
            s.position(Position::Absolute)
                .inset_top(4.0)
                .inset_bottom(5.0)
                .width_full()
                .z_index(10)
        })
        .draggable()
        .on_event(EventListener::PointerDown, select)
        .on_event_stop(EventListener::DragStart, drag_start)
        .on_event_stop(EventListener::DragEnd, drop_end);

    let track_resizer = track_resizer
        .style(|s| {
            s.position(Position::Absolute)
                .inset_bottom(-5.0)
                .width_full()
                .height(10.0)
                .z_index(10)
                .cursor(CursorStyle::RowResize)
        })
        .draggable()
        .on_event(EventListener::DragStart, resize_start)
        .on_event(EventListener::PointerMove, resize_move)
        .on_event_stop(EventListener::DragEnd, resize_end);

    let tcp_view = tcp_view
        .style(move |s| s.height(track_height.get() - 1.0))
        .keyboard_navigatable()
        .style(move |s| {
            s.apply_if(state.selection.get() == Some(node), |s| {
                s.background(Color::rgba8(255, 0, 0, 20))
            })
        });

    v_stack((
        before_marker,
        v_stack((track_resizer, track_selector, tcp_view, inside_marker))
            .style(|s| {
                s.position(Position::Relative)
                    .outline(1.0)
                    .outline_color(Color::BLACK)
                    .border_color(Color::BLACK)
                    .margin_bottom(1.0)
            })
            .on_event(EventListener::DragOver, drag_over),
        after_marker,
    ))
    .debug_name("TcpTrackNode")
    .style(move |s| {
        s.margin_left(20.0 * (node.level as f32))
            .position(Position::Relative)
    })
}

fn tav_tree_node<B: Backend>(state: State, node: TrackNode, is_even: bool) -> impl IntoView {
    let track_height = create_memo(move |_| {
        state.track_heights.with(|v| {
            v.get(&node)
                .map(|v| v.get())
                .unwrap_or(state.min_track_height)
        })
    });

    track_arrangement_view::<B>(node.id, is_even)
        .style(move |s| s.width_full().height(track_height.get()))
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
    .style(move |s| s.padding(10))
}

fn track_arrangement_view<B: Backend>(_id: TrackId, is_even: bool) -> impl IntoView {
    label(move || "Arrangement view...").style(move |s| {
        s.width_full()
            .background(Color::BLACK.with_alpha_factor(if is_even { 0.03 } else { 0.1 }))
    })
}
