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
use futures_lite::{Stream, StreamExt};
use rdaw_api::{Backend, TrackEvent};
use rdaw_object::TrackId;
use rdaw_ui_kit::{button, ColorKind, Level, Theme};

fn track_view<B: Backend>(id: TrackId) -> impl IntoView {
    let name = RwSignal::new(String::new());
    let editor_name = RwSignal::new(String::new());

    fetch(
        move |back: Arc<B>| async move { back.get_track_name(id).await },
        move |res| {
            name.set(res.unwrap());
        },
    );

    subscribe(
        move |back: Arc<B>| async move { back.subscribe_track(id).await },
        move |event| match event {
            TrackEvent::NameChanged { new_name } => name.set(new_name),
            _ => {}
        },
    );

    create_effect(move |old| {
        let editor_name = editor_name.get();
        let name = name.get();

        if old.is_none() || old.is_some_and(|v| v == editor_name) || editor_name == name {
            return editor_name;
        };

        let name_clone = editor_name.clone();

        fetch(
            move |back: Arc<B>| async move { back.set_track_name(id, editor_name).await },
            move |res| res.unwrap(),
        );

        name_clone
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
            fetch(
                move |back: Arc<B>| async move { back.create_track("Unnamed".into()).await },
                move |res| {
                    let id = res.unwrap();
                    tracks.update(|tracks| tracks.push(id));
                },
            );

            EventPropagation::Stop
        }),
        button(ColorKind::Surface, Level::Mid, || "Refresh").on_click(move |_| {
            fetch(
                move |back: Arc<B>| async move { back.list_tracks().await },
                move |res| {
                    tracks.set(res.unwrap());
                },
            );

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

pub fn spawn(future: impl Future<Output = ()> + Send + 'static) {
    let executor = use_context::<Arc<Executor>>().unwrap();
    executor.spawn(future).detach();
}

pub fn fetch<B, T, Fac, Fut, Cb>(factory: Fac, on_completed: Cb)
where
    B: Backend,
    T: Send + 'static,
    Fac: FnOnce(Arc<B>) -> Fut,
    Fut: Future<Output = T> + Send + 'static,
    Cb: Fn(T) + 'static,
{
    let backend = use_context::<Arc<B>>().unwrap();
    let future = factory(backend);

    let send = create_ext_action(Scope::new(), move |v| {
        on_completed(v);
    });

    spawn(async move {
        send(future.await);
    });
}

pub fn subscribe<B, T, Fac, Fut, Str, Cb>(factory: Fac, on_message: Cb)
where
    B: Backend,
    T: Send + 'static,
    Fac: FnOnce(Arc<B>) -> Fut,
    Fut: Future<Output = rdaw_api::Result<Str>> + Send + 'static,
    Str: Stream<Item = T> + Send + 'static,
    Cb: Fn(T) + 'static,
{
    fn next<T, Str, Cb>(mut stream: Pin<Box<Str>>, on_message: Rc<Cb>)
    where
        T: Send + 'static,
        Str: Stream<Item = T> + Send + 'static,
        Cb: Fn(T) + 'static,
    {
        let callback = on_message.clone();

        let send_next = create_ext_action(Scope::new(), move |(stream, value)| {
            callback(value);
            next(stream, on_message);
        });

        spawn(async move {
            let Some(value) = Pin::new(&mut stream).next().await else {
                return;
            };

            send_next((stream, value));
        });
    }

    let backend = use_context::<Arc<B>>().unwrap();
    let future = factory(backend);

    let send_stream = create_ext_action(Scope::new(), move |stream| {
        next(Box::pin(stream), Rc::new(on_message));
    });

    spawn(async move {
        send_stream(future.await.unwrap());
    });
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
