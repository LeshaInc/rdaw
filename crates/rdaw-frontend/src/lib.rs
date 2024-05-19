pub mod api;
mod track;

use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use async_executor::Executor;
use floem::ext_event::create_ext_action;
use floem::keyboard::{Key, Modifiers, NamedKey};
use floem::reactive::{provide_context, use_context, Scope};
use floem::views::{h_stack, Decorators};
use floem::{IntoView, View};
use futures_lite::future::block_on;
use futures_lite::StreamExt;
use rdaw_api::{Backend, BoxStream, TrackId};
use rdaw_ui_kit::Theme;
use track::track_tree_view;

fn app_view<B: Backend>(master_track: TrackId) -> impl IntoView {
    h_stack((
        track_tree_view::<B>(master_track, false).style(|s| s.flex_grow(1.0)),
        track_tree_view::<B>(master_track, true).style(|s| s.flex_grow(1.0)),
    ))
    .style(|s| s.width_full())
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

    provide_context(backend.clone());
    provide_context(executor.clone());

    Theme::light().provide();

    std::thread::spawn(move || {
        block_on(async move {
            loop {
                executor.tick().await;
            }
        })
    });

    let master_track = block_on(async move { backend.create_track().await }).unwrap();

    floem::launch(move || {
        let view = app_view::<B>(master_track)
            .keyboard_navigatable()
            .into_view();
        let id = view.id();
        view.on_key_down(Key::Named(NamedKey::F11), Modifiers::empty(), move |_| {
            id.inspect()
        })
    });
}
