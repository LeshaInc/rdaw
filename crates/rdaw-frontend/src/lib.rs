pub mod api;
pub mod views;

use std::collections::VecDeque;
use std::future::Future;
use std::sync::{Arc, Mutex};

use floem::ext_event::{create_ext_action, register_ext_trigger};
use floem::keyboard::{Key, Modifiers, NamedKey};
use floem::reactive::{provide_context, use_context, with_scope, RwSignal, Scope};
use floem::views::{dyn_container, Decorators};
use floem::{IntoView, View};
use futures::executor::{block_on, ThreadPool};
use futures::task::SpawnExt;
use futures::StreamExt;
use rdaw_api::arrangement::ArrangementId;
use rdaw_api::document::DocumentId;
use rdaw_api::{Backend, BoxStream, Error};
use rdaw_ui_kit::Theme;

pub fn app_view(document_id: DocumentId, main_arrangement: ArrangementId) -> impl IntoView {
    provide_document_id(document_id);

    views::arrangement(main_arrangement)
        .style(|s| s.width_full().height_full())
        .window_scale(move || 1.0)
}

pub fn get_document_id() -> DocumentId {
    use_context().expect("no document id in scope")
}

pub fn provide_document_id(id: DocumentId) {
    provide_context(id);
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

    let handle = executor
        .spawn_with_handle(async move {
            send(future.await);
        })
        .unwrap();

    scope.create_rw_signal(handle);
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

    let (document_id, main_arrangement) = block_on(async move {
        let document_id = backend.create_document().await?;
        let main_arrangement = backend.get_document_arrangement(document_id).await?;
        Ok::<_, Error>((document_id, main_arrangement))
    })
    .unwrap();

    floem::launch(move || {
        let state = RwSignal::new((document_id, main_arrangement));

        let view = dyn_container(move || state.get(), move |(doc, arr)| app_view(doc, arr))
            .style(|s| s.width_full().height_full())
            .keyboard_navigatable()
            .into_view();

        let id = view.id();

        view.on_key_down(Key::Named(NamedKey::F11), Modifiers::empty(), move |_| {
            id.inspect()
        })
        .on_key_down(Key::Named(NamedKey::F1), Modifiers::empty(), move |_| {
            let document_id = state.get().0;
            api::call(
                move |api| async move {
                    api.save_document_as(document_id, "/tmp/test.rdaw".into())
                        .await
                },
                drop,
            );
        })
        .on_key_down(Key::Named(NamedKey::F2), Modifiers::empty(), move |_| {
            api::call(
                move |api| async move {
                    let document_id = api.open_document("/tmp/test.rdaw".into()).await?;
                    let arrangement_id = api.get_document_arrangement(document_id).await?;
                    Ok((document_id, arrangement_id))
                },
                move |new_state| state.set(new_state),
            );
        })
    });
}
