use std::path::PathBuf;
use std::sync::Arc;

use floem::reactive::use_context;
use rdaw_api::arrangement::{ArrangementEvent, ArrangementId};
use rdaw_api::blob::BlobId;
use rdaw_api::tempo_map::TempoMapId;
use rdaw_api::time::Time;
use rdaw_api::track::{
    TrackEvent, TrackHierarchy, TrackHierarchyEvent, TrackId, TrackItem, TrackItemId,
    TrackViewEvent, TrackViewId, TrackViewItem,
};
use rdaw_api::{Backend, Error};

use crate::{spawn, stream_for_each};

pub fn get_backend() -> Arc<dyn Backend> {
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
        pub fn $method($($arg: $ArgTy,)* callback: impl FnOnce($RetTy) + 'static) {
            let backend = get_backend();
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
        pub fn $method($($arg: $ArgTy,)*) {
            let backend = get_backend();
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
        pub fn $method(
            $($arg: $ArgTy,)*
            callback: impl Fn($RetTy) + 'static,
        ) {
            let backend = get_backend();
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
    fn list_arrangements() -> Vec<ArrangementId>;

    fn create_arrangement() -> ArrangementId;

    #[sub]
    fn subscribe_arrangement(id: ArrangementId) -> ArrangementEvent;

    fn get_arrangement_name(id: ArrangementId) -> String;

    fn set_arrangement_name(id: ArrangementId, name: String);

    fn get_arrangement_main_track(id: ArrangementId) -> TrackId;

    fn get_arrangement_tempo_map(id: ArrangementId) -> TempoMapId;
}

generate_methods! {
    fn create_internal_blob(data: Vec<u8>) -> BlobId;

    fn create_external_blob(path: PathBuf) -> BlobId;
}

generate_methods! {
    fn list_tracks() -> Vec<TrackId>;

    fn create_track() -> TrackId;

    #[sub]
    fn subscribe_track(id: TrackId) -> TrackEvent;

    #[sub]
    fn subscribe_track_hierarchy(id: TrackId) -> TrackHierarchyEvent;

    #[sub]
    fn subscribe_track_view(view_id: TrackViewId) -> TrackViewEvent;

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

    fn add_track_item(track_id: TrackId, item: TrackItem) -> TrackItemId;

    fn get_track_item(track_id: TrackId, item_id: TrackItemId) -> TrackItem;

    fn remove_track_item(track_id: TrackId, item_id: TrackItemId);

    fn move_track_item(
        track_id: TrackId,
        item_id: TrackItemId,
        new_position: Time,
    );

    fn resize_track_item(
        track_id: TrackId,
        item_id: TrackItemId,
        new_duration: Time,
    );

    fn get_track_view_item(
        view_id: TrackViewId,
        item_id: TrackItemId,
    ) -> TrackViewItem;

    fn get_track_view_range(
        view_id: TrackViewId,
        start: Option<Time>,
        end: Option<Time>,
    ) -> Vec<(TrackItemId, TrackViewItem)>;
}
