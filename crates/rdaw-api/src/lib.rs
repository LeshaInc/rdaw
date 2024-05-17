use futures_lite::Stream;
use rdaw_object::TrackId;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid ID")]
    InvalidId,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[trait_variant::make(Send)]
pub trait TrackOperations {
    async fn create_track(&mut self, name: String) -> Result<TrackId>;

    async fn subscribe_track(&mut self, id: TrackId) -> Result<impl Stream<Item = TrackEvent>>;

    async fn get_track_name(&self, id: TrackId) -> Result<String>;

    async fn set_track_name(&mut self, id: TrackId, name: String) -> Result<()>;
}

#[derive(Debug, Clone)]
pub enum TrackEvent {
    NameChanged(String),
}
