mod subscribers;

use futures_lite::Stream;
use rdaw_api::{Error, Result, TrackEvent, TrackOperations};
use rdaw_object::{BeatMap, Hub, Track, TrackId};

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

    async fn set_track_name(&mut self, id: TrackId, name: String) -> Result<()> {
        let track = self.hub.tracks.get_mut(id).ok_or(Error::InvalidId)?;
        track.name.clone_from(&name);

        let event = TrackEvent::NameChanged(name);
        self.track_subscribers.notify(id, event).await;

        Ok(())
    }
}
