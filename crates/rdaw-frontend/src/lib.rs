pub mod api;
pub mod views;

use std::sync::Arc;

use floem::keyboard::{Key, Modifiers, NamedKey};
use floem::peniko::Color;
use floem::reactive::{provide_context, use_context, RwSignal};
use floem::views::{dyn_container, h_stack, scroll, Decorators};
use floem::{IntoView, View};
use futures::executor::{block_on, ThreadPool};
use rdaw_api::arrangement::ArrangementId;
use rdaw_api::document::DocumentId;
use rdaw_api::{Backend, Error};
use rdaw_ui::task::provide_executor;
use rdaw_ui::theme::Theme;
use rdaw_ui::views::tree::{tree, FsTreeModel};
use views::arrangement;

pub fn app_view(document_id: DocumentId, main_arrangement: ArrangementId) -> impl IntoView {
    provide_document_id(document_id);

    h_stack((
        scroll(tree(FsTreeModel::new("/".into()))).style(|s| {
            s.min_width(400.0)
                .max_width(400.0)
                .border_right(1.0)
                .border_color(Color::BLACK)
        }),
        arrangement(main_arrangement),
    ))
    .style(|s| s.width_full().height_full())
    .window_scale(move || 1.0)
}

pub fn get_document_id() -> DocumentId {
    use_context().expect("no document id in scope")
}

pub fn provide_document_id(id: DocumentId) {
    provide_context(id);
}

pub fn run(backend: Arc<dyn Backend>) {
    let executor = Arc::new(ThreadPool::builder().pool_size(1).create().unwrap());

    provide_executor(executor.clone());
    provide_context(backend.clone());
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
