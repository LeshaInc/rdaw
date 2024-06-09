use floem::event::{Event, EventListener, EventPropagation};
use floem::kurbo::Vec2;
use floem::peniko::Color;
use floem::reactive::{batch, create_memo, RwSignal};
use floem::style::CursorStyle;
use floem::taffy::{Display, Position};
use floem::views::{
    container, dyn_container, empty, h_stack, scroll, v_stack, virtual_stack, Decorators,
    VirtualDirection, VirtualItemSize, VirtualVector,
};
use floem::{IntoView, View};
use rdaw_api::arrangement::ArrangementId;
use rdaw_api::track::{TrackHierarchy, TrackHierarchyEvent, TrackId, TrackNode};
use rdaw_core::collections::{HashMap, HashSet, ImVec};

use crate::api;
use crate::views::{track_control, track_items};

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

pub fn arrangement(id: ArrangementId) -> impl IntoView {
    let main_track = RwSignal::new(None);

    api::get_arrangement_main_track(id, move |id| {
        main_track.set(Some(id));
    });

    dyn_container(move || match main_track.get() {
        Some(id) => track_tree(id).into_any(),
        None => empty().into_any(),
    })
    .style(|s| s.width_full().height_full())
}

fn track_tree(root: TrackId) -> impl IntoView {
    let state = State {
        selection: RwSignal::new(None),
        transitive_selection: RwSignal::new(HashSet::default()),
        is_dragging: RwSignal::new(false),
        drop_location: RwSignal::new(DropLocation::Forbidden),
        hierarchy: RwSignal::new(TrackHierarchy::new(root)),
        min_track_height: 50.0,
        track_heights: RwSignal::new(HashMap::default()),
    };

    api::get_track_hierarchy(root, move |new_hierarchy| {
        state.hierarchy.set(new_hierarchy);
    });

    api::subscribe_track_hierarchy(root, move |event| {
        let TrackHierarchyEvent::ChildrenChanged { id, new_children } = event else {
            return;
        };

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
                .get(node)
                .map(|signal| signal.get())
                .unwrap_or(state.min_track_height)
        })
    };

    let control_tree = virtual_stack(
        VirtualDirection::Vertical,
        VirtualItemSize::Fn(Box::new(get_height)),
        move || order.get(),
        move |node| *node,
        move |node| track_control_node(state, node),
    );

    let items_tree = virtual_stack(
        VirtualDirection::Vertical,
        VirtualItemSize::Fn(Box::new(move |(_, node)| get_height(node))),
        move || order.get().enumerate(),
        move |(idx, node)| (*node, idx % 2 == 0),
        move |(idx, node)| track_items_node(state, node, idx % 2 == 0),
    );

    let scroll_delta = RwSignal::new(Vec2::ZERO);

    scroll(
        h_stack((
            control_tree.style(|s| s.width(400.0)),
            items_tree.style(|s| s.flex_grow(1.0)).on_event(
                EventListener::PointerWheel,
                move |ev| {
                    let Event::PointerWheel(ev) = ev else {
                        return EventPropagation::Continue;
                    };

                    if ev.modifiers.shift() {
                        scroll_delta.set(ev.delta);
                        return EventPropagation::Stop;
                    }

                    EventPropagation::Continue
                },
            ),
        ))
        .style(|s| s.width_full()),
    )
    .style(|s| s.width_full().height_full())
    .scroll_delta(move || scroll_delta.get())
    .debug_name("TrackTree")
}

fn track_control_node(state: State, node: TrackNode) -> impl IntoView {
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

    let control_view = track_control(node.id).into_view();
    let control_view_id = control_view.id();

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

        let Some(size) = control_view_id.get_size() else {
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

        api::move_track(old_parent, old_index, new_parent, new_index);
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

    let control_view = control_view
        .style(move |s| s.height(track_height.get() - 1.0))
        .keyboard_navigatable()
        .style(move |s| {
            s.apply_if(state.selection.get() == Some(node), |s| {
                s.background(Color::rgba8(255, 0, 0, 20))
            })
        });

    v_stack((
        before_marker,
        v_stack((track_resizer, track_selector, control_view, inside_marker))
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
    .debug_name("TrackControlNode")
    .style(move |s| {
        s.margin_left(20.0 * (node.level as f32))
            .position(Position::Relative)
    })
}

fn track_items_node(state: State, node: TrackNode, is_even: bool) -> impl IntoView {
    let track_height = create_memo(move |_| {
        state.track_heights.with(|v| {
            v.get(&node)
                .map(|v| v.get())
                .unwrap_or(state.min_track_height)
        })
    });

    container(track_items(node.id, is_even))
        .debug_name("TrackItemsNode")
        .style(move |s| s.width_full().height(track_height.get()))
}
