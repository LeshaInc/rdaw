use rdaw_api::arrangement::{ArrangementEvent, ArrangementId, ArrangementOperations};
use rdaw_api::tempo_map::TempoMapId;
use rdaw_api::track::TrackId;
use rdaw_api::{BoxStream, Error, Result};
use slotmap::Key;
use tracing::instrument;

use super::Arrangement;
use crate::tempo_map::TempoMap;
use crate::track::Track;
use crate::{Backend, BackendHandle};

crate::dispatch::define_dispatch_ops! {
    pub enum ArrangementOperation;

    impl Backend {
        pub fn dispatch_arrangement_operation;
    }

    impl ArrangementOperations for BackendHandle;

    ListArrangements => list_arrangements() -> Result<Vec<ArrangementId>>;

    CreateArrangement => create_arrangement() -> Result<ArrangementId>;

    SubscribeArrangement => subscribe_arrangement(
        id: ArrangementId,
    ) -> Result<BoxStream<ArrangementEvent>>;

    GetArrangementName => get_arrangement_name(
        id: ArrangementId,
    ) -> Result<String>;

    SetArrangementName => set_arrangement_name(
        id: ArrangementId,
        name: String,
    ) -> Result<()>;

    GetArrangementMainTrack => get_arrangement_main_track(
        id: ArrangementId,
    ) -> Result<TrackId>;

    GetArrangementTempoMap => get_arrangement_tempo_map(
        id: ArrangementId,
    ) -> Result<TempoMapId>;
}

impl Backend {
    #[instrument(skip_all, err)]
    pub fn list_arrangements(&self) -> Result<Vec<ArrangementId>> {
        let arrangements = self.hub.arrangements.iter().map(|(id, _)| id).collect();
        Ok(arrangements)
    }

    #[instrument(skip_all, err)]
    pub fn create_arrangement(&mut self) -> Result<ArrangementId> {
        let tempo_map = TempoMap::new(120.0, 4);
        let tempo_map_id = self.hub.tempo_maps.insert(tempo_map);

        let main_track = Track::new(tempo_map_id, "main".into());
        let main_track_id = self.hub.tracks.insert(main_track);

        let arrangement = Arrangement::new(tempo_map_id, main_track_id, String::new());
        let arrangement_id = self.hub.arrangements.insert(arrangement);

        let mut id_str = format!("{:?}", arrangement_id.data());
        if let Some(v) = id_str.find('v') {
            id_str.truncate(v);
        }

        self.hub.arrangements[arrangement_id].name = format!("Arrangement {id_str}");

        Ok(arrangement_id)
    }

    #[instrument(skip_all, err)]
    pub fn subscribe_arrangement(
        &mut self,
        id: ArrangementId,
    ) -> Result<BoxStream<ArrangementEvent>> {
        if !self.hub.arrangements.contains_id(id) {
            return Err(Error::InvalidId);
        }

        Ok(Box::pin(self.subscribers.arrangement.subscribe(id)))
    }

    #[instrument(skip_all, err)]
    pub fn get_arrangement_name(&self, id: ArrangementId) -> Result<String> {
        let arrangement = self.hub.arrangements.get(id).ok_or(Error::InvalidId)?;
        Ok(arrangement.name.clone())
    }

    #[instrument(skip_all, err)]
    pub fn set_arrangement_name(&mut self, id: ArrangementId, new_name: String) -> Result<()> {
        let arrangement = self.hub.arrangements.get_mut(id).ok_or(Error::InvalidId)?;
        arrangement.name.clone_from(&new_name);

        let event = ArrangementEvent::NameChanged { new_name };
        self.subscribers.arrangement.notify(id, event);

        Ok(())
    }

    #[instrument(skip_all, err)]
    pub fn get_arrangement_main_track(&self, id: ArrangementId) -> Result<TrackId> {
        let arrangement = self.hub.arrangements.get(id).ok_or(Error::InvalidId)?;
        Ok(arrangement.main_track_id)
    }

    #[instrument(skip_all, err)]
    pub fn get_arrangement_tempo_map(&self, id: ArrangementId) -> Result<TempoMapId> {
        let arrangement = self.hub.arrangements.get(id).ok_or(Error::InvalidId)?;
        Ok(arrangement.tempo_map_id)
    }
}
