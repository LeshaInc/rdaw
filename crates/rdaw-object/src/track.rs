use rstar::{RTree, RTreeObject, AABB};
use slotmap::SlotMap;

use crate::{BeatMap, ItemId, RealTime, Time};

slotmap::new_key_type! {
    pub struct TrackId;

    pub struct TrackItemId;
}

#[derive(Debug, Clone)]
pub struct Track {
    beat_map: BeatMap,
    items: SlotMap<TrackItemId, TrackItem>,
    items_tree: RTree<TreeItem>,
}

impl Track {
    pub fn new(beat_map: BeatMap) -> Track {
        Track {
            beat_map,
            items: SlotMap::default(),
            items_tree: RTree::new(),
        }
    }

    pub fn insert(&mut self, item_id: ItemId, position: Time, duration: Time) -> TrackItemId {
        let real_start = position.to_real(&self.beat_map);
        let real_end = real_start + duration.to_real(&self.beat_map);

        let item = TrackItem {
            inner: item_id,
            position,
            duration,
            real_start,
            real_end,
        };

        let id = self.items.insert(item);

        self.items_tree
            .insert(TreeItem::new(id, real_start, real_end));

        id
    }

    pub fn remove(&mut self, id: TrackItemId) {
        if let Some(item) = self.items.remove(id) {
            self.items_tree
                .remove(&TreeItem::new(id, item.real_start, item.real_end));
        }
    }

    pub fn get(&self, id: TrackItemId) -> Option<&TrackItem> {
        self.items.get(id)
    }

    pub fn get_mut(&mut self, id: TrackItemId) -> Option<&mut TrackItem> {
        self.items.get_mut(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (TrackItemId, &TrackItem)> + '_ {
        self.items.iter()
    }

    pub fn range(
        &self,
        start: Option<Time>,
        end: Option<Time>,
    ) -> impl Iterator<Item = (TrackItemId, &TrackItem)> + '_ {
        let start = start.map_or(RealTime::MIN, |t| t.to_real(&self.beat_map));
        let end = end.map_or(RealTime::MAX, |t| t.to_real(&self.beat_map));

        let envelope = AABB::from_corners((start.as_nanos(), 0), (end.as_nanos(), 0));

        self.items_tree
            .locate_in_envelope_intersecting(&envelope)
            .map(|item| (item.id, &self.items[item.id]))
    }

    pub fn move_item(&mut self, id: TrackItemId, new_pos: Time) {
        let item = &mut self.items[id];

        let duration = item.real_duration();
        let old_start = item.real_start;
        let old_end = item.real_end;

        item.position = new_pos;
        item.real_start = new_pos.to_real(&self.beat_map);
        item.real_end = item.real_start + duration;

        let new_start = item.real_start;
        let new_end = item.real_end;

        self.items_tree
            .remove(&TreeItem::new(id, old_start, old_end));
        self.items_tree
            .insert(TreeItem::new(id, new_start, new_end));
    }

    pub fn resize_item(&mut self, id: TrackItemId, new_duration: Time) {
        self.items[id].duration = new_duration;
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
        AABB::from_corners((self.start.as_nanos(), 0), (self.end.as_nanos() as i64, 0))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TrackItem {
    inner: ItemId,
    position: Time,
    duration: Time,
    real_start: RealTime,
    real_end: RealTime,
}

impl TrackItem {
    pub fn inner_id(&self) -> ItemId {
        self.inner
    }

    pub fn position(&self) -> Time {
        self.position
    }

    pub fn duration(&self) -> Time {
        self.duration
    }

    pub fn real_start(&self) -> RealTime {
        self.real_start
    }

    pub fn real_end(&self) -> RealTime {
        self.real_end
    }

    pub fn real_duration(&self) -> RealTime {
        self.real_end - self.real_start
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use slotmap::KeyData;

    use super::*;
    use crate::item::AudioItemId;

    fn item_id() -> ItemId {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let raw = KeyData::from_ffi(COUNTER.fetch_add(1, Ordering::Relaxed));
        ItemId::Audio(AudioItemId::from(raw))
    }

    #[test]
    fn test_simple() {
        let beat_map = BeatMap {
            beats_per_minute: 120.0,
            beats_per_bar: 4,
        };

        let mut track = Track::new(beat_map);

        let inner = item_id();
        let id = track.insert(
            inner,
            Time::Real(RealTime::from_secs_f64(1.0)),
            Time::Real(RealTime::from_secs_f64(2.0)),
        );

        assert_eq!(
            track.get(id),
            Some(&TrackItem {
                inner,
                position: Time::Real(RealTime::from_secs_f64(1.0)),
                duration: Time::Real(RealTime::from_secs_f64(2.0)),
                real_start: RealTime::from_secs_f64(1.0),
                real_end: RealTime::from_secs_f64(3.0),
            })
        );

        assert_eq!(track.iter().count(), 1);
        assert_eq!(track.range(None, None).count(), 1);

        track.remove(id);

        assert_eq!(track.get(id), None);
        assert_eq!(track.iter().count(), 0);
        assert_eq!(track.range(None, None).count(), 0);
    }

    #[test]
    fn test_range() {
        let beat_map = BeatMap {
            beats_per_minute: 120.0,
            beats_per_bar: 4,
        };

        let mut track = Track::new(beat_map);

        let real_0s = Time::Real(RealTime::from_secs_f64(0.0));
        let real_1s = Time::Real(RealTime::from_secs_f64(1.0));
        let real_2s = Time::Real(RealTime::from_secs_f64(2.0));
        let real_3s = Time::Real(RealTime::from_secs_f64(3.0));
        let real_5s = Time::Real(RealTime::from_secs_f64(5.0));

        let id1 = track.insert(item_id(), real_0s, real_2s);
        let id2 = track.insert(item_id(), real_1s, real_3s);
        let id3 = track.insert(item_id(), real_2s, real_3s);

        let find = |start, end| {
            let mut items = track
                .range(start, end)
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