use rdaw_api::arrangement::{
    ArrangementId, ArrangementOperations, ArrangementRequest, ArrangementResponse,
};
use rdaw_api::document::DocumentId;
use rdaw_api::tempo_map::TempoMapId;
use rdaw_api::track::TrackId;
use rdaw_api::{BackendProtocol, Result};
use rdaw_rpc::StreamId;
use slotmap::Key;
use tracing::instrument;

use super::Arrangement;
use crate::object::ObjectKey;
use crate::tempo_map::TempoMap;
use crate::track::Track;
use crate::Backend;

#[rdaw_rpc::handler(protocol = BackendProtocol, operations = ArrangementOperations)]
impl Backend {
    #[instrument(level = "trace", skip_all, err)]
    #[handler]
    pub fn create_arrangement(&mut self, document_id: DocumentId) -> Result<ArrangementId> {
        let tempo_map = TempoMap::new(120.0);
        let tempo_map_id = self
            .hub
            .tempo_maps
            .insert(ObjectKey::new_random(document_id), tempo_map);

        let main_track = Track::new("Main Track".into());
        let main_track_id = self
            .hub
            .tracks
            .insert(ObjectKey::new_random(document_id), main_track);

        let arrangement = Arrangement {
            tempo_map_id,
            main_track_id,
            name: String::new(),
        };

        let arrangement_id = self
            .hub
            .arrangements
            .insert(ObjectKey::new_random(document_id), arrangement);

        let mut id_str = format!("{:?}", arrangement_id.data());
        if let Some(v) = id_str.find('v') {
            id_str.truncate(v);
        }

        self.hub.arrangements[arrangement_id].name = format!("Arrangement {id_str}");

        Ok(arrangement_id)
    }

    #[instrument(level = "trace", skip_all, err)]
    #[handler]
    pub fn subscribe_arrangement_name(&mut self, id: ArrangementId) -> Result<StreamId> {
        self.hub.arrangements.ensure_has(id)?;
        Ok(self.subscribers.arrangement_name.subscribe(id))
    }

    #[instrument(level = "trace", skip_all, err)]
    #[handler]
    pub fn get_arrangement_name(&self, id: ArrangementId) -> Result<String> {
        let arrangement = self.hub.arrangements.get_or_err(id)?;
        Ok(arrangement.name.clone())
    }

    #[instrument(level = "trace", skip_all, err)]
    #[handler]
    pub fn set_arrangement_name(&mut self, id: ArrangementId, new_name: String) -> Result<()> {
        let arrangement = self.hub.arrangements.get_mut_or_err(id)?;
        arrangement.name.clone_from(&new_name);
        self.subscribers.arrangement_name.notify(id, new_name);
        Ok(())
    }

    #[instrument(level = "trace", skip_all, err)]
    #[handler]
    pub fn get_arrangement_main_track(&self, id: ArrangementId) -> Result<TrackId> {
        let arrangement = self.hub.arrangements.get_or_err(id)?;
        Ok(arrangement.main_track_id)
    }

    #[instrument(level = "trace", skip_all, err)]
    #[handler]
    pub fn get_arrangement_tempo_map(&self, id: ArrangementId) -> Result<TempoMapId> {
        let arrangement = self.hub.arrangements.get_or_err(id)?;
        Ok(arrangement.tempo_map_id)
    }
}
