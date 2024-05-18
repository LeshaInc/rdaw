mod blob;
mod subscribers;
mod track;

use blob::BlobCache;
use rdaw_api::TrackEvent;
use rdaw_object::{Hub, TrackId};

use self::subscribers::Subscribers;

#[derive(Debug, Default)]
pub struct Backend {
    hub: Hub,
    blob_cache: BlobCache,
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
