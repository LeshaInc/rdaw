use rdaw_api::{ItemId, TempoMapId, Time, TrackId, TrackItem, TrackItemId};
use rdaw_core::collections::HashSet;
use rdaw_core::time::RealTime;
use rstar::{RTree, RTreeObject, AABB};
use slotmap::SlotMap;

use crate::{Hub, Object, TempoMap, Uuid};

#[derive(Debug, Clone)]
pub struct Track {
    uuid: Uuid,
    pub name: String,
    pub links: TrackLinks,
    pub items: TrackItems,
    pub tempo_map_id: TempoMapId,
}

impl Track {
    pub fn new(tempo_map_id: TempoMapId, name: String) -> Track {
        Track {
            uuid: Uuid::new_v4(),
            name,
            links: TrackLinks::default(),
            items: TrackItems::new(),
            tempo_map_id,
        }
    }
}

impl Object for Track {
    type Id = TrackId;

    fn uuid(&self) -> Uuid {
        self.uuid
    }

    fn trace<F: FnMut(Uuid)>(&self, hub: &Hub, callback: &mut F) {
        callback(self.uuid);

        self.links.trace(hub, callback);
        self.items.trace(hub, callback);
    }
}

#[derive(Debug, Clone, Default)]
pub struct TrackLinks {
    pub children: Vec<TrackId>,
    pub ancestors: HashSet<TrackId>,
    pub direct_ancestors: HashSet<TrackId>,
}

impl TrackLinks {
    fn trace<F: FnMut(Uuid)>(&self, hub: &Hub, callback: &mut F) {
        for &child_id in &self.children {
            if let Some(child) = hub.tracks.get(child_id) {
                child.trace(hub, callback);
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TrackItems {
    items: SlotMap<TrackItemId, TrackItem>,
    tree: RTree<TreeItem>,
}

impl TrackItems {
    pub fn new() -> TrackItems {
        TrackItems::default()
    }

    pub fn insert(
        &mut self,
        tempo_map: &TempoMap,
        item_id: ItemId,
        position: Time,
        duration: Time,
    ) -> TrackItemId {
        let real_start = tempo_map.to_real(position);
        let real_end = real_start + tempo_map.to_real(duration);

        let item = TrackItem {
            inner: item_id,
            position,
            duration,
            real_start,
            real_end,
        };

        let id = self.items.insert(item);
        self.tree.insert(TreeItem::new(id, real_start, real_end));
        id
    }

    pub fn remove(&mut self, id: TrackItemId) {
        if let Some(item) = self.items.remove(id) {
            self.tree
                .remove(&TreeItem::new(id, item.real_start, item.real_end));
        }
    }

    pub fn get(&self, id: TrackItemId) -> Option<&TrackItem> {
        self.items.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (TrackItemId, &TrackItem)> + '_ {
        self.items.iter()
    }

    pub fn range(
        &self,
        tempo_map: &TempoMap,
        start: Option<Time>,
        end: Option<Time>,
    ) -> impl Iterator<Item = (TrackItemId, &TrackItem)> + '_ {
        let start = start.map_or(RealTime::MIN, |t| tempo_map.to_real(t));
        let end = end.map_or(RealTime::MAX, |t| tempo_map.to_real(t));

        let envelope = AABB::from_corners((start.as_nanos(), 0), (end.as_nanos(), 0));

        self.tree
            .locate_in_envelope_intersecting(&envelope)
            .map(|item| (item.id, &self.items[item.id]))
    }

    fn update_item_envelope(&mut self, id: TrackItemId, mut func: impl FnMut(&mut TrackItem)) {
        let item = &mut self.items[id];

        let old_start = item.real_start;
        let old_end = item.real_end;

        func(item);

        let new_start = item.real_start;
        let new_end = item.real_end;

        if old_start == new_start && old_end == new_end {
            return;
        }

        self.tree.remove(&TreeItem::new(id, old_start, old_end));
        self.tree.insert(TreeItem::new(id, new_start, new_end));
    }

    pub fn move_item(&mut self, tempo_map: &TempoMap, id: TrackItemId, new_pos: Time) {
        self.update_item_envelope(id, |item| {
            let duration = item.real_duration();
            item.position = new_pos;
            item.real_start = tempo_map.to_real(new_pos);
            item.real_end = item.real_start + duration;
        });
    }

    pub fn resize_item(&mut self, tempo_map: &TempoMap, id: TrackItemId, new_duration: Time) {
        self.update_item_envelope(id, |item| {
            item.duration = new_duration;
            item.real_end = item.real_start + tempo_map.to_real(new_duration);
        });
    }

    fn trace<F: FnMut(Uuid)>(&self, hub: &Hub, callback: &mut F) {
        for item in self.items.values() {
            match item.inner {
                ItemId::Audio(id) => {
                    if let Some(item) = hub.audio_items.get(id) {
                        item.trace(hub, callback);
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TreeItem {
    id: TrackItemId,
    start: RealTime,
    end: RealTime,
}

impl TreeItem {
    fn new(id: TrackItemId, start: RealTime, end: RealTime) -> TreeItem {
        TreeItem { id, start, end }
    }
}

impl RTreeObject for TreeItem {
    type Envelope = AABB<(i64, i64)>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners((self.start.as_nanos(), 0), (self.end.as_nanos(), 0))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use rdaw_api::AudioItemId;
    use slotmap::KeyData;

    use super::*;

    fn item_id() -> ItemId {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let raw = KeyData::from_ffi(COUNTER.fetch_add(1, Ordering::Relaxed));
        ItemId::Audio(AudioItemId::from(raw))
    }

    #[test]
    fn test_simple() {
        let tempo_map = TempoMap {
            beats_per_minute: 120.0,
            beats_per_bar: 4,
        };

        let mut items = TrackItems::default();

        let inner = item_id();
        let id = items.insert(
            &tempo_map,
            inner,
            Time::Real(RealTime::from_secs_f64(1.0)),
            Time::Real(RealTime::from_secs_f64(2.0)),
        );

        assert_eq!(
            items.get(id),
            Some(&TrackItem {
                inner,
                position: Time::Real(RealTime::from_secs_f64(1.0)),
                duration: Time::Real(RealTime::from_secs_f64(2.0)),
                real_start: RealTime::from_secs_f64(1.0),
                real_end: RealTime::from_secs_f64(3.0),
            })
        );

        assert_eq!(items.iter().count(), 1);
        assert_eq!(items.range(&tempo_map, None, None).count(), 1);

        items.remove(id);

        assert_eq!(items.get(id), None);
        assert_eq!(items.iter().count(), 0);
        assert_eq!(items.range(&tempo_map, None, None).count(), 0);
    }

    #[test]
    fn test_range() {
        let tempo_map = TempoMap {
            beats_per_minute: 120.0,
            beats_per_bar: 4,
        };

        let mut items = TrackItems::default();

        let real_0s = Time::Real(RealTime::from_secs_f64(0.0));
        let real_1s = Time::Real(RealTime::from_secs_f64(1.0));
        let real_2s = Time::Real(RealTime::from_secs_f64(2.0));
        let real_3s = Time::Real(RealTime::from_secs_f64(3.0));
        let real_5s = Time::Real(RealTime::from_secs_f64(5.0));

        let id1 = items.insert(&tempo_map, item_id(), real_0s, real_2s);
        let id2 = items.insert(&tempo_map, item_id(), real_1s, real_3s);
        let id3 = items.insert(&tempo_map, item_id(), real_2s, real_3s);

        let find = |start, end| {
            let mut items = items
                .range(&tempo_map, start, end)
                .map(|(id, _)| id)
                .collect::<Vec<_>>();
            items.sort_unstable();
            items
        };

        assert_eq!(find(None, None), vec![id1, id2, id3]);
        assert_eq!(find(Some(real_0s), Some(real_3s)), vec![id1, id2, id3]);
        assert_eq!(find(Some(real_0s), Some(real_0s)), vec![id1]);
        assert_eq!(find(Some(real_0s), Some(real_1s)), vec![id1, id2]);
        assert_eq!(find(Some(real_3s), Some(real_3s)), vec![id2, id3]);
        assert_eq!(find(Some(real_5s), Some(real_5s)), vec![id3]);
    }
}
