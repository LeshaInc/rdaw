use rdaw_api::arrangement::ArrangementId;
use rdaw_api::time::Time;
use rdaw_api::track::{TrackId, TrackItem, TrackItemId, TrackViewId, TrackViewItem};
use rdaw_core::collections::HashMap;
use rdaw_core::time::RealTime;
use rstar::{RTree, RTreeObject, AABB};
use slotmap::SecondaryMap;

use super::Track;
use crate::storage::Hub;
use crate::tempo_map::TempoMap;

#[derive(Debug, Default)]
pub struct TrackViewCache {
    views: HashMap<TrackId, HashMap<ArrangementId, TrackView>>,
}

impl TrackViewCache {
    pub fn iter(&self, track_id: TrackId) -> impl Iterator<Item = (TrackViewId, &TrackView)> + '_ {
        self.views.get(&track_id).into_iter().flat_map(move |v| {
            v.iter().map(move |(k, v)| {
                (
                    TrackViewId {
                        track_id,
                        arrangement_id: *k,
                    },
                    v,
                )
            })
        })
    }

    pub fn iter_mut(
        &mut self,
        track_id: TrackId,
    ) -> impl Iterator<Item = (TrackViewId, &mut TrackView)> + '_ {
        self.views
            .get_mut(&track_id)
            .into_iter()
            .flat_map(move |v| {
                v.iter_mut().map(move |(k, v)| {
                    (
                        TrackViewId {
                            track_id,
                            arrangement_id: *k,
                        },
                        v,
                    )
                })
            })
    }

    pub fn get_or_insert(&mut self, hub: &Hub, view_id: TrackViewId) -> &mut TrackView {
        self.views
            .entry(view_id.track_id)
            .or_default()
            .entry(view_id.arrangement_id)
            .or_insert_with(|| {
                let track = &hub.tracks[view_id.track_id];
                let arrangement = &hub.arrangements[view_id.arrangement_id];
                let tempo_map = &hub.tempo_maps[arrangement.tempo_map_id];
                TrackView::new(track, tempo_map)
            })
    }
}

#[derive(Debug, Clone, Default)]
pub struct TrackView {
    items: SecondaryMap<TrackItemId, TrackViewItem>,
    tree: RTree<TreeItem>,
}

impl TrackView {
    pub fn new(track: &Track, tempo_map: &TempoMap) -> TrackView {
        let mut track_view = TrackView::default();
        track_view.compute(track, tempo_map);
        track_view
    }

    pub fn compute(&mut self, track: &Track, tempo_map: &TempoMap) {
        self.items.clear();
        self.items.set_capacity(track.items.capacity());

        for (item_id, item) in &track.items {
            let real_start = tempo_map.to_real(item.start);
            let real_duration = tempo_map.to_real(item.duration); // TODO: handle non-constant tempo
            let real_end = real_start + real_duration;

            let view_item = TrackViewItem {
                inner: item.inner,
                start: item.start,
                duration: item.duration,
                real_start,
                real_end,
            };

            self.items.insert(item_id, view_item);
        }

        let tree_items = self
            .items
            .iter()
            .map(|(id, view_item)| TreeItem {
                id,
                start: view_item.real_start,
                end: view_item.real_end,
            })
            .collect();

        self.tree = RTree::bulk_load(tree_items);
    }

    pub fn add_item(
        &mut self,
        tempo_map: &TempoMap,
        item_id: TrackItemId,
        item: TrackItem,
    ) -> TrackViewItem {
        let real_start = tempo_map.to_real(item.start);
        let real_duration = tempo_map.to_real(item.duration); // TODO: handle non-constant tempo
        let real_end = real_start + real_duration;

        let view_item = TrackViewItem {
            inner: item.inner,
            start: item.start,
            duration: item.duration,
            real_start,
            real_end,
        };

        self.items.insert(item_id, view_item);
        self.tree.insert(TreeItem {
            id: item_id,
            start: real_start,
            end: real_end,
        });

        view_item
    }

    pub fn get_item(&self, item_id: TrackItemId) -> Option<&TrackViewItem> {
        self.items.get(item_id)
    }

    pub fn contains_item(&self, item_id: TrackItemId) -> bool {
        self.items.contains_key(item_id)
    }

    pub fn remove_item(&mut self, item_id: TrackItemId) {
        if let Some(item) = self.items.remove(item_id) {
            self.tree
                .remove(&TreeItem::new(item_id, item.real_start, item.real_end));
        }
    }

    pub fn get_range(
        &self,
        tempo_map: &TempoMap,
        start: Option<Time>,
        end: Option<Time>,
    ) -> impl Iterator<Item = (TrackItemId, &TrackViewItem)> + '_ {
        let start = start.map_or(RealTime::MIN, |t| tempo_map.to_real(t));
        let end = end.map_or(RealTime::MAX, |t| tempo_map.to_real(t));

        let envelope = AABB::from_corners((start.as_nanos(), 0), (end.as_nanos(), 0));

        self.tree
            .locate_in_envelope_intersecting(&envelope)
            .map(|item| (item.id, &self.items[item.id]))
    }

    fn update_item_envelope<T>(
        &mut self,
        id: TrackItemId,
        mut func: impl FnMut(&mut TrackViewItem) -> T,
    ) -> T {
        let item = &mut self.items[id];

        let old_start = item.real_start;
        let old_end = item.real_end;

        let res = func(item);

        let new_start = item.real_start;
        let new_end = item.real_end;

        if old_start == new_start && old_end == new_end {
            return res;
        }

        self.tree.remove(&TreeItem::new(id, old_start, old_end));
        self.tree.insert(TreeItem::new(id, new_start, new_end));

        res
    }

    pub fn move_item(
        &mut self,
        tempo_map: &TempoMap,
        id: TrackItemId,
        new_start: Time,
    ) -> RealTime {
        self.update_item_envelope(id, |item| {
            let duration = item.real_duration();
            item.start = new_start;
            item.real_start = tempo_map.to_real(new_start);
            item.real_end = item.real_start + duration;
            item.real_start
        })
    }

    pub fn resize_item(
        &mut self,
        tempo_map: &TempoMap,
        id: TrackItemId,
        new_duration: Time,
    ) -> RealTime {
        self.update_item_envelope(id, |item| {
            item.duration = new_duration;
            item.real_end = item.real_start + tempo_map.to_real(new_duration);
            item.real_duration()
        })
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

    use rdaw_api::item::{AudioItemId, ItemId};
    use slotmap::{KeyData, SlotMap};

    use super::*;

    fn item_id() -> ItemId {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let raw = KeyData::from_ffi(COUNTER.fetch_add(1, Ordering::Relaxed));
        ItemId::Audio(AudioItemId::from(raw))
    }

    #[test]
    fn test_simple() {
        let tempo_map = TempoMap::new(120.0);
        let mut items = SlotMap::default();
        let mut view = TrackView::default();

        let item = TrackItem {
            inner: item_id(),
            start: Time::Real(RealTime::from_secs_f64(1.0)),
            duration: Time::Real(RealTime::from_secs_f64(2.0)),
        };
        let id = items.insert(item);

        let view_item = view.add_item(&tempo_map, id, item);

        assert_eq!(
            view_item,
            TrackViewItem {
                inner: item.inner,
                start: item.start,
                duration: item.duration,
                real_start: RealTime::from_secs_f64(1.0),
                real_end: RealTime::from_secs_f64(3.0),
            }
        );

        assert_eq!(view.get_range(&tempo_map, None, None).count(), 1);

        view.remove_item(id);

        assert_eq!(view.get_item(id), None);
        assert_eq!(view.get_range(&tempo_map, None, None).count(), 0);
    }

    #[test]
    fn test_range() {
        let tempo_map = TempoMap::new(120.0);
        let mut items = SlotMap::default();
        let mut view = TrackView::default();

        let real_0s = Time::Real(RealTime::from_secs_f64(0.0));
        let real_1s = Time::Real(RealTime::from_secs_f64(1.0));
        let real_2s = Time::Real(RealTime::from_secs_f64(2.0));
        let real_3s = Time::Real(RealTime::from_secs_f64(3.0));
        let real_5s = Time::Real(RealTime::from_secs_f64(5.0));

        let item1 = TrackItem {
            inner: item_id(),
            start: real_0s,
            duration: real_2s,
        };

        let item2 = TrackItem {
            inner: item_id(),
            start: real_1s,
            duration: real_3s,
        };

        let item3 = TrackItem {
            inner: item_id(),
            start: real_2s,
            duration: real_3s,
        };

        let id1 = items.insert(item1);
        let id2 = items.insert(item2);
        let id3 = items.insert(item3);

        view.add_item(&tempo_map, id1, item1);
        view.add_item(&tempo_map, id2, item2);
        view.add_item(&tempo_map, id3, item3);

        let find = |start, end| {
            let mut items = view
                .get_range(&tempo_map, start, end)
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
