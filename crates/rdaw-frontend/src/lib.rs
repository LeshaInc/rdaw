pub mod api;
pub mod views;

use std::collections::VecDeque;
use std::future::Future;
use std::sync::{Arc, Mutex};

use floem::ext_event::{create_ext_action, register_ext_trigger};
use floem::keyboard::{Key, Modifiers, NamedKey};
use floem::reactive::{provide_context, use_context, with_scope, Scope};
use floem::views::Decorators;
use floem::{IntoView, View};
use futures::executor::{block_on, ThreadPool};
use futures::StreamExt;
use rdaw_api::arrangement::ArrangementId;
use rdaw_api::{Backend, BoxStream};
use rdaw_ui_kit::Theme;

pub fn app_view(main_arrangement: ArrangementId) -> impl IntoView {
    views::arrangement(main_arrangement)
        .style(|s| s.width_full().height_full())
        .window_scale(move || 1.0)
}

pub fn spawn<T: Send + 'static>(
    future: impl Future<Output = T> + Send + 'static,
    on_completed: impl FnOnce(T) + 'static,
) {
    let scope = Scope::current();
    let executor = use_context::<Arc<ThreadPool>>().unwrap();

    let child = scope.create_child();
    let send = create_ext_action(scope, move |v| {
        with_scope(child, move || {
            on_completed(v);
        });
    });

    scope.create_rw_signal(executor.spawn_ok(async move {
        send(future.await);
    }));
}

pub fn stream_for_each<T: Send + 'static>(
    mut stream: BoxStream<T>,
    on_message: impl Fn(T) + 'static,
) {
    let scope = Scope::current();
    let queue = Arc::new(Mutex::new(VecDeque::new()));

    let trigger = scope.create_trigger();
    trigger.notify();

    let queue_clone = queue.clone();
    scope.create_effect(move |_| {
        trigger.track();
        if let Ok(mut queue) = queue_clone.lock() {
            while let Some(value) = queue.pop_front() {
                on_message(value);
            }
        }
    });

    spawn(
        async move {
            while let Some(value) = stream.next().await {
                if let Ok(mut queue) = queue.lock() {
                    queue.push_back(value);
                }

                register_ext_trigger(trigger);
            }
        },
        drop,
    );
}

pub fn run(backend: Arc<dyn Backend>) {
    let executor = Arc::new(ThreadPool::builder().pool_size(1).create().unwrap());

    provide_context(backend.clone());
    provide_context(executor.clone());

    Theme::light().provide();

    let main_arrangement = block_on(async move { backend.create_arrangement().await }).unwrap();

    floem::launch(move || {
        let view = app_view(main_arrangement)
            .keyboard_navigatable()
            .into_view();
        let id = view.id();
        view.on_key_down(Key::Named(NamedKey::F11), Modifiers::empty(), move |_| {
            id.inspect()
        })
    });
}
