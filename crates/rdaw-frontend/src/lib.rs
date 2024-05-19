pub mod api;

use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use async_executor::Executor;
use floem::event::EventPropagation;
use floem::ext_event::create_ext_action;
use floem::peniko::Color;
use floem::reactive::{create_effect, provide_context, use_context, RwSignal, Scope};
use floem::taffy::FlexDirection;
use floem::views::{dyn_stack, h_stack, text_input, v_stack, Decorators};
use floem::IntoView;
use futures_lite::future::block_on;
use futures_lite::StreamExt;
use rdaw_api::{Backend, BoxStream, TrackEvent, TrackId};
use rdaw_ui_kit::{button, ColorKind, Level, Theme};

fn track_view<B: Backend>(id: TrackId) -> impl IntoView {
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

fn tracks_view<B: Backend>() -> impl IntoView {
    let tracks = RwSignal::new(Vec::<TrackId>::new());

    v_stack((
        button(ColorKind::Surface, Level::Mid, || "Add track").on_click(move |_| {
            api::create_track::<B>("Unnamed".into(), move |id| {
                tracks.update(|tracks| tracks.push(id));
            });

            EventPropagation::Stop
        }),
        button(ColorKind::Surface, Level::Mid, || "Refresh").on_click(move |_| {
            api::list_tracks::<B>(move |res| {
                tracks.set(res);
            });

            EventPropagation::Stop
        }),
        dyn_stack(
            move || tracks.get(),
            move |id| *id,
            move |id| track_view::<B>(id),
        )
        .style(|s| s.flex_direction(FlexDirection::Column)),
    ))
}

fn app_view<B: Backend>() -> impl IntoView {
    h_stack((tracks_view::<B>(), tracks_view::<B>())).style(|s| s.gap(32, 32))
}

pub fn spawn<T: Send + 'static>(
    future: impl Future<Output = T> + Send + 'static,
    on_completed: impl FnOnce(T) + 'static,
) {
    let executor = use_context::<Arc<Executor>>().unwrap();

    let send = create_ext_action(Scope::new(), move |v| {
        on_completed(v);
    });

    executor
        .spawn(async move {
            send(future.await);
        })
        .detach();
}

pub fn stream_for_each<T: Send + 'static>(stream: BoxStream<T>, on_message: impl Fn(T) + 'static) {
    fn next<T: Send + 'static>(mut stream: BoxStream<T>, on_message: Rc<impl Fn(T) + 'static>) {
        spawn(
            async move {
                let Some(value) = Pin::new(&mut stream).next().await else {
                    return None;
                };

                Some((stream, value))
            },
            move |v| {
                if let Some((stream, value)) = v {
                    on_message(value);
                    next(stream, on_message.clone());
                }
            },
        );
    }

    next(stream, Rc::new(on_message));
}

pub fn run<B: Backend>(backend: B) {
    let backend = Arc::new(backend);
    let executor = Arc::new(Executor::new());

    provide_context(backend);
    provide_context(executor.clone());

    Theme::light().provide();

    std::thread::spawn(move || {
        block_on(async move {
            loop {
                executor.tick().await;
            }
        })
    });

    floem::launch(app_view::<B>);
}
