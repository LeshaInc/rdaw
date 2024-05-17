mod subscribers;

use futures_lite::Stream;
use rdaw_api::{Error, Result, TrackEvent, TrackOperations};
use rdaw_object::{BeatMap, Hub, ItemId, Time, Track, TrackId, TrackItem, TrackItemId};

use self::subscribers::Subscribers;

#[derive(Debug, Default)]
pub struct Backend {
    hub: Hub,
    track_subscribers: Subscribers<TrackId, TrackEvent>,
}

impl Backend {
    pub fn new() -> Backend {
        Backend::default()
    }

    pub fn cleanup(&mut self) {
        self.track_subscribers.cleanup();
    }
}

impl TrackOperations for Backend {
    async fn create_track(&mut self, name: String) -> Result<TrackId> {
        // TODO: remove this
        let beat_map = BeatMap {
            beats_per_minute: 120.0,
            beats_per_bar: 4,
        };

        let track = Track::new(beat_map, name);
        let id = self.hub.tracks.insert(track);
        Ok(id)
    }

    async fn subscribe_track(&mut self, id: TrackId) -> Result<impl Stream<Item = TrackEvent>> {
        if !self.hub.tracks.contains_id(id) {
            return Err(Error::InvalidId);
        }

        Ok(self.track_subscribers.subscribe(id))
    }

    async fn get_track_name(&self, id: TrackId) -> Result<String> {
        let track = self.hub.tracks.get(id).ok_or(Error::InvalidId)?;
        Ok(track.name.clone())
    }

    async fn set_track_name(&mut self, id: TrackId, new_name: String) -> Result<()> {
        let track = self.hub.tracks.get_mut(id).ok_or(Error::InvalidId)?;
        track.name.clone_from(&new_name);

        let event = TrackEvent::NameChanged { new_name };
        self.track_subscribers.notify(id, event).await;

        Ok(())
    }

    async fn get_track_range(
        &self,
        id: TrackId,
        start: Option<Time>,
        end: Option<Time>,
    ) -> Result<Vec<TrackItemId>> {
        let track = self.hub.tracks.get(id).ok_or(Error::InvalidId)?;
        let items = track.range(start, end).map(|(id, _)| id).collect();
        Ok(items)
    }

    async fn add_track_item(
        &mut self,
        id: TrackId,
        item_id: ItemId,
        position: Time,
        duration: Time,
    ) -> Result<TrackItemId> {
        let track = self.hub.tracks.get_mut(id).ok_or(Error::InvalidId)?;
        let item_id = track.insert(item_id, position, duration);

        let item = track.get(item_id).ok_or(Error::InvalidId)?;
        let event = TrackEvent::ItemAdded {
            id: item_id,
            start: item.real_start(),
            end: item.real_end(),
        };
        self.track_subscribers.notify(id, event).await;

        Ok(item_id)
    }

    async fn get_track_item(&self, id: TrackId, item_id: TrackItemId) -> Result<TrackItem> {
        let track = self.hub.tracks.get(id).ok_or(Error::InvalidId)?;
        let item = track.get(item_id).ok_or(Error::InvalidId)?;
        Ok(item.clone())
    }

    async fn remove_track_item(&mut self, id: TrackId, item_id: TrackItemId) -> Result<()> {
        let track = self.hub.tracks.get_mut(id).ok_or(Error::InvalidId)?;
        track.remove(item_id);

        let event = TrackEvent::ItemRemoved { id: item_id };
        self.track_subscribers.notify(id, event).await;

        Ok(())
    }

    async fn move_track_item(
        &mut self,
        id: TrackId,
        item_id: TrackItemId,
        new_position: Time,
    ) -> Result<()> {
        let track = self.hub.tracks.get_mut(id).ok_or(Error::InvalidId)?;
        track.move_item(item_id, new_position);

        let item = track.get(item_id).ok_or(Error::InvalidId)?;
        let event = TrackEvent::ItemMoved {
            id: item_id,
            new_start: item.real_start(),
        };
        self.track_subscribers.notify(id, event).await;

        Ok(())
    }

    async fn resize_track_item(
        &mut self,
        id: TrackId,
        item_id: TrackItemId,
        new_duration: Time,
    ) -> Result<()> {
        let track = self.hub.tracks.get_mut(id).ok_or(Error::InvalidId)?;
        track.move_item(item_id, new_duration);

        let item = track.get(item_id).ok_or(Error::InvalidId)?;
        let new_duration = item.real_duration();

        let event = TrackEvent::ItemResized {
            id: item_id,
            new_duration,
        };
        self.track_subscribers.notify(id, event).await;

        Ok(())
    }
}
