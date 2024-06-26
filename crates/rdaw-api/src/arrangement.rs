use crate::document::DocumentId;
use crate::tempo_map::TempoMapId;
use crate::track::TrackId;
use crate::{BackendProtocol, BoxStream, Result};

slotmap::new_key_type! {
    pub struct ArrangementId;
}

#[rdaw_rpc::operations(protocol = BackendProtocol)]
pub trait ArrangementOperations {
    async fn create_arrangement(&self, document_id: DocumentId) -> Result<ArrangementId>;

    #[sub]
    async fn subscribe_arrangement_name(&self, id: ArrangementId) -> Result<BoxStream<String>>;

    async fn get_arrangement_name(&self, id: ArrangementId) -> Result<String>;

    async fn set_arrangement_name(&self, id: ArrangementId, new_name: String) -> Result<()>;

    async fn get_arrangement_main_track(&self, id: ArrangementId) -> Result<TrackId>;

    async fn get_arrangement_tempo_map(&self, id: ArrangementId) -> Result<TempoMapId>;
}
