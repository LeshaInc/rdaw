use std::sync::Arc;

use rdaw_api::arrangement::{ArrangementEvent, ArrangementEvents, ArrangementId};
use rdaw_api::track::{
    TrackEvent, TrackEvents, TrackHierarchyEvent, TrackId, TrackViewEvent, TrackViewId,
};
use rdaw_api::{BackendProtocol, Result};
use rdaw_rpc::transport::ServerTransport;
use rdaw_rpc::{StreamId, StreamIdAllocator, Subscribers};

use super::{Object, Storage};
use crate::arrangement::Arrangement;
use crate::asset::Asset;
use crate::item::AudioItem;
use crate::source::AudioSource;
use crate::tempo_map::TempoMap;
use crate::track::Track;

#[derive(Debug, Default)]
pub struct Hub {
    pub arrangements: Storage<Arrangement>,
    pub assets: Storage<Asset>,
    pub audio_items: Storage<AudioItem>,
    pub audio_sources: Storage<AudioSource>,
    pub tempo_maps: Storage<TempoMap>,
    pub tracks: Storage<Track>,
}

impl Hub {
    pub fn storage<T: StorageRef>(&self) -> &Storage<T> {
        T::storage_ref(self)
    }

    pub fn storage_mut<T: StorageRef>(&mut self) -> &mut Storage<T> {
        T::storage_ref_mut(self)
    }
}

pub trait StorageRef: Object + Sized {
    fn storage_ref(hub: &Hub) -> &Storage<Self>;

    fn storage_ref_mut(hub: &mut Hub) -> &mut Storage<Self>;
}

macro_rules! impl_storage_ref {
    ($field:ident: $ty:ty) => {
        impl StorageRef for $ty {
            fn storage_ref(hub: &Hub) -> &Storage<Self> {
                &hub.$field
            }

            fn storage_ref_mut(hub: &mut Hub) -> &mut Storage<Self> {
                &mut hub.$field
            }
        }
    };
}

impl_storage_ref!(arrangements: Arrangement);
impl_storage_ref!(assets: Asset);
impl_storage_ref!(audio_items: AudioItem);
impl_storage_ref!(audio_sources: AudioSource);
impl_storage_ref!(tempo_maps: TempoMap);
impl_storage_ref!(tracks: Track);

#[derive(Debug)]
pub struct SubscribersHub {
    pub arrangement: Subscribers<ArrangementId, ArrangementEvent>,
    pub track: Subscribers<TrackId, TrackEvent>,
    pub track_hierarchy: Subscribers<TrackId, TrackHierarchyEvent>,
    pub track_view: Subscribers<TrackViewId, TrackViewEvent>,
}

impl SubscribersHub {
    pub fn new(id_allocator: Arc<StreamIdAllocator>) -> SubscribersHub {
        SubscribersHub {
            arrangement: Subscribers::new(id_allocator.clone()),
            track: Subscribers::new(id_allocator.clone()),
            track_hierarchy: Subscribers::new(id_allocator.clone()),
            track_view: Subscribers::new(id_allocator.clone()),
        }
    }

    pub fn close_one(&mut self, stream: StreamId) {
        if let Some(key) = self.arrangement.find_key(stream) {
            self.arrangement.close_one(key, stream);
        }

        if let Some(key) = self.track.find_key(stream) {
            self.track.close_one(key, stream);
        }

        if let Some(key) = self.track_hierarchy.find_key(stream) {
            self.track_hierarchy.close_one(key, stream);
        }

        if let Some(key) = self.track_view.find_key(stream) {
            self.track_view.close_one(key, stream);
        }
    }

    pub async fn deliver<T>(&mut self, t: &T) -> Result<()>
    where
        T: ServerTransport<BackendProtocol>,
    {
        self.arrangement
            .deliver(t, |ev| ArrangementEvents::SubscribeArrangement(ev).into())
            .await?;

        self.track
            .deliver(t, |ev| TrackEvents::SubscribeTrack(ev).into())
            .await?;

        self.track_hierarchy
            .deliver(t, |ev| TrackEvents::SubscribeTrackHierarchy(ev).into())
            .await?;

        self.track_view
            .deliver(t, |ev| TrackEvents::SubscribeTrackView(ev).into())
            .await?;

        Ok(())
    }
}
