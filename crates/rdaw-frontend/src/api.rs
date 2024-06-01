use std::path::PathBuf;
use std::sync::Arc;

use floem::reactive::use_context;
use rdaw_api::blob::BlobId;
use rdaw_api::item::ItemId;
use rdaw_api::time::Time;
use rdaw_api::track::{
    TrackEvent, TrackHierarchy, TrackHierarchyEvent, TrackId, TrackItem, TrackItemId,
};
use rdaw_api::{Backend, Error};

use crate::{spawn, stream_for_each};

pub fn get_backend<B: Backend>() -> Arc<B> {
    use_context().expect("no backend in scope")
}

#[cold]
fn handle_error(error: Error) {
    tracing::error!(%error);
}

macro_rules! generate_method {
    {
        fn $method:ident($($arg:ident: $ArgTy:ty),* $(,)?) -> $RetTy:ty;
    } => {
        pub fn $method<B: Backend>($($arg: $ArgTy,)* callback: impl FnOnce($RetTy) + 'static) {
            let backend = get_backend::<B>();
            spawn(
                async move { backend.$method($($arg,)*).await },
                move |res| match res {
                    Ok(v) => callback(v),
                    Err(e) => handle_error(e),
                }
            );
        }
    };

    {
        fn $method:ident($($arg:ident: $ArgTy:ty),* $(,)?);
    } => {
        pub fn $method<B: Backend>($($arg: $ArgTy,)*) {
            let backend = get_backend::<B>();
            spawn(
                async move { backend.$method($($arg,)*).await },
                move |res| if let Err(e) = res {
                    handle_error(e);
                }
            );
        }
    };

    {
        #[sub] fn $method:ident($($arg:ident: $ArgTy:ty),* $(,)?) -> $RetTy:ty;
    } => {
        pub fn $method<B: Backend>(
            $($arg: $ArgTy,)*
            callback: impl Fn($RetTy) + 'static,
        ) {
            let backend = get_backend::<B>();
            spawn(
                async move { backend.$method($($arg,)*).await },
                move |stream| match stream {
                    Ok(stream) => {
                        stream_for_each(stream, callback);
                    }
                    Err(e) => handle_error(e),
                },
            );
        }
    };

}

macro_rules! generate_methods {
    {
        $(
            $(#[$kind:ident])? fn $method:ident($($arg:ident: $ArgTy:ty),* $(,)?) $(-> $RetTy:ty)?;
        )*
    } => {
        $(
            generate_method!(
                $(#[$kind])? fn $method($($arg: $ArgTy,)*) $(-> $RetTy)?;
            );
        )*
    };
}

generate_methods! {
    fn list_tracks() -> Vec<TrackId>;

    fn create_track() -> TrackId;

    #[sub]
    fn subscribe_track(id: TrackId) -> TrackEvent;

    #[sub]
    fn subscribe_track_hierarchy(id: TrackId) -> TrackHierarchyEvent;

    fn get_track_name(id: TrackId) -> String;

    fn set_track_name(id: TrackId, name: String);

    fn get_track_children(parent: TrackId) -> Vec<TrackId>;

    fn get_track_hierarchy(root: TrackId) -> TrackHierarchy;

    fn append_track_child(parent: TrackId, child: TrackId);

    fn insert_track_child(
        parent: TrackId,
        child: TrackId,
        index: usize,
    );

    fn move_track(
        old_parent: TrackId,
        old_index: usize,
        new_parent: TrackId,
        new_index: usize,
    );

    fn remove_track_child(parent: TrackId, index: usize);

    fn get_track_range(
        id: TrackId,
        start: Option<Time>,
        end: Option<Time>,
    ) -> Vec<TrackItemId>;

    fn add_track_item(
        id: TrackId,
        item_id: ItemId,
        position: Time,
        duration: Time,
    ) -> TrackItemId;

    fn get_track_item(id: TrackId, item_id: TrackItemId) -> TrackItem;

    fn remove_track_item(id: TrackId, item_id: TrackItemId);

    fn move_track_item(
        id: TrackId,
        item_id: TrackItemId,
        new_position: Time,
    );

    fn resize_track_item(
        id: TrackId,
        item_id: TrackItemId,
        new_duration: Time,
    );
}

generate_methods! {
    fn create_internal_blob(data: Vec<u8>) -> BlobId;

    fn create_external_blob(path: PathBuf) -> BlobId;
}
