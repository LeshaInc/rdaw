use std::collections::VecDeque;
use std::future::Future;
use std::sync::{Arc, Mutex};

use floem::ext_event::{create_ext_action, register_ext_trigger};
use floem::reactive::{provide_context, use_context, with_scope, Scope};
use futures::executor::ThreadPool;
use futures::task::SpawnExt;
use futures::{Stream, StreamExt};

pub fn provide_executor(executor: Arc<ThreadPool>) {
    provide_context(executor);
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
    mut stream: impl Stream<Item = T> + Send + Unpin + 'static,
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
