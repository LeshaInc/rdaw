use crate::tempo_map::TempoMapId;
use crate::track::TrackId;
use crate::{BoxStream, Result};

slotmap::new_key_type! {
    pub struct ArrangementId;
}

#[rdaw_macros::api_operations]
pub trait ArrangementOperations {
    async fn list_arrangements(&self) -> Result<Vec<ArrangementId>>;

    async fn create_arrangement(&self) -> Result<ArrangementId>;

    #[sub]
    async fn subscribe_arrangement(&self, id: ArrangementId)
        -> Result<BoxStream<ArrangementEvent>>;

    async fn get_arrangement_name(&self, id: ArrangementId) -> Result<String>;

    async fn set_arrangement_name(&self, id: ArrangementId, name: String) -> Result<()>;

    async fn get_arrangement_main_track(&self, id: ArrangementId) -> Result<TrackId>;

    async fn get_arrangement_tempo_map(&self, id: ArrangementId) -> Result<TempoMapId>;
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum ArrangementEvent {
    NameChanged { new_name: String },
}
