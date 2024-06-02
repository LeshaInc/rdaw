use futures_lite::future::block_on;
use futures_lite::StreamExt;
use rdaw_api::track::{TrackEvent, TrackHierarchyEvent, TrackId, TrackNode};
use rdaw_api::{Error, Result};
use slotmap::KeyData;

use crate::Backend;

fn invalid_track_id() -> TrackId {
    TrackId::from(KeyData::from_ffi(u64::MAX))
}

#[test]
fn list_tracks() -> Result<()> {
    let mut backend = Backend::new();

    let tracks = backend.list_tracks()?;
    assert!(tracks.is_empty());

    let track1 = backend.create_track()?;
    let track2 = backend.create_track()?;

    let mut tracks = backend.list_tracks()?;
    tracks.sort_unstable();
    assert_eq!(tracks.len(), 2);
    assert_eq!(tracks, vec![track1, track2]);

    Ok(())
}

#[test]
fn create_track() -> Result<()> {
    let mut backend = Backend::new();

    let track1 = backend.create_track()?;
    let track2 = backend.create_track()?;
    assert!(track1 != track2);

    Ok(())
}

#[test]
fn subscribe_track() -> Result<()> {
    let mut backend = Backend::new();

    assert!(matches!(
        backend.subscribe_track(invalid_track_id()),
        Err(Error::InvalidId),
    ));

    let track = backend.create_track()?;
    let mut stream = backend.subscribe_track(track)?;

    backend.set_track_name(track, "New name".into())?;

    block_on(async move {
        backend.update().await;

        assert_eq!(
            stream.next().await,
            Some(TrackEvent::NameChanged {
                new_name: "New name".into()
            })
        );
    });

    Ok(())
}

#[test]
fn subscribe_track_hierarchy() -> Result<()> {
    let mut backend = Backend::new();

    assert!(matches!(
        backend.subscribe_track_hierarchy(invalid_track_id()),
        Err(Error::InvalidId),
    ));

    let root = backend.create_track()?;
    let mut stream = backend.subscribe_track_hierarchy(root)?;

    let child1 = backend.create_track()?;
    let child2 = backend.create_track()?;
    backend.append_track_child(root, child1)?;
    backend.append_track_child(root, child2)?;

    let grandchild = backend.create_track()?;
    backend.append_track_child(child1, grandchild)?;

    block_on(async move {
        backend.update().await;

        assert_eq!(
            stream.next().await,
            Some(TrackHierarchyEvent::ChildrenChanged {
                id: root,
                new_children: vec![child1].into()
            })
        );

        assert_eq!(
            stream.next().await,
            Some(TrackHierarchyEvent::ChildrenChanged {
                id: root,
                new_children: vec![child1, child2].into()
            })
        );

        assert_eq!(
            stream.next().await,
            Some(TrackHierarchyEvent::ChildrenChanged {
                id: child1,
                new_children: vec![grandchild].into()
            })
        );
    });

    Ok(())
}

#[test]
#[ignore = "not yet implemented"]
fn subscribe_track_view() -> Result<()> {
    todo!()
}

#[test]
fn get_set_track_name() -> Result<()> {
    let mut backend = Backend::new();

    assert!(matches!(
        backend.get_track_name(invalid_track_id()),
        Err(Error::InvalidId),
    ));

    let track = backend.create_track()?;
    assert_eq!(backend.get_track_name(track)?, "Track 1");
    backend.set_track_name(track, "New name".into())?;
    assert_eq!(backend.get_track_name(track)?, "New name");

    Ok(())
}

#[test]
fn get_track_children() -> Result<()> {
    let mut backend = Backend::new();

    assert!(matches!(
        backend.get_track_children(invalid_track_id()),
        Err(Error::InvalidId),
    ));

    let root = backend.create_track()?;
    let child1 = backend.create_track()?;
    let child2 = backend.create_track()?;
    backend.append_track_child(root, child1)?;
    backend.append_track_child(root, child2)?;
    assert_eq!(backend.get_track_children(root)?, vec![child1, child2]);

    Ok(())
}

#[test]
fn get_track_hierarchy() -> Result<()> {
    let mut backend = Backend::new();

    assert!(matches!(
        backend.get_track_hierarchy(invalid_track_id()),
        Err(Error::InvalidId),
    ));

    let root = backend.create_track()?;
    let child1 = backend.create_track()?;
    let child2 = backend.create_track()?;
    let grandchild = backend.create_track()?;
    backend.append_track_child(root, child1)?;
    backend.append_track_child(root, child2)?;
    backend.append_track_child(child1, grandchild)?;

    let hierarchy = backend.get_track_hierarchy(child1)?;
    assert_eq!(hierarchy.root(), child1);
    assert_eq!(
        hierarchy.children(child1).collect::<Vec<_>>(),
        vec![grandchild]
    );

    let hierarchy = backend.get_track_hierarchy(child2)?;
    assert_eq!(hierarchy.root(), child2);
    assert_eq!(hierarchy.children(child2).count(), 0);

    let hierarchy = backend.get_track_hierarchy(root)?;
    assert_eq!(hierarchy.root(), root);
    assert_eq!(
        hierarchy.children(root).collect::<Vec<_>>(),
        vec![child1, child2]
    );
    assert_eq!(
        hierarchy.children(child1).collect::<Vec<_>>(),
        vec![grandchild]
    );
    assert_eq!(hierarchy.children(child2).count(), 0,);
    assert_eq!(hierarchy.children(grandchild).count(), 0,);

    let mut nodes = Vec::new();
    hierarchy.dfs(root, |node| nodes.push(node));

    assert_eq!(
        nodes,
        vec![
            TrackNode {
                id: root,
                index: 0,
                level: 0,
                parent: None
            },
            TrackNode {
                id: child1,
                index: 0,
                level: 1,
                parent: Some(root)
            },
            TrackNode {
                id: grandchild,
                index: 0,
                level: 2,
                parent: Some(child1)
            },
            TrackNode {
                id: child2,
                index: 1,
                level: 1,
                parent: Some(root)
            }
        ]
    );

    Ok(())
}

#[test]
fn append_track_child() -> Result<()> {
    let mut backend = Backend::new();

    assert!(matches!(
        backend.append_track_child(invalid_track_id(), invalid_track_id()),
        Err(Error::InvalidId),
    ));

    let parent = backend.create_track()?;

    assert!(matches!(
        backend.append_track_child(parent, invalid_track_id()),
        Err(Error::InvalidId),
    ));

    let child = backend.create_track()?;

    assert!(matches!(
        backend.append_track_child(invalid_track_id(), child),
        Err(Error::InvalidId),
    ));

    backend.append_track_child(parent, child)?;
    assert_eq!(backend.get_track_children(parent)?, vec![child]);

    Ok(())
}

#[test]
fn insert_track_child() -> Result<()> {
    let mut backend = Backend::new();

    assert!(matches!(
        backend.insert_track_child(invalid_track_id(), invalid_track_id(), 0),
        Err(Error::InvalidId),
    ));

    let parent = backend.create_track()?;

    assert!(matches!(
        backend.insert_track_child(parent, invalid_track_id(), 0),
        Err(Error::InvalidId),
    ));

    let child = backend.create_track()?;

    assert!(matches!(
        backend.insert_track_child(invalid_track_id(), child, 0),
        Err(Error::InvalidId),
    ));

    assert!(matches!(
        backend.insert_track_child(parent, child, 1),
        Err(Error::IndexOutOfBounds),
    ));

    backend.insert_track_child(parent, child, 0)?;
    assert_eq!(backend.get_track_children(parent)?, vec![child]);

    let child0 = backend.create_track()?;
    backend.insert_track_child(parent, child0, 0)?;
    assert_eq!(backend.get_track_children(parent)?, vec![child0, child]);

    Ok(())
}

#[test]
fn move_track() -> Result<()> {
    let mut backend = Backend::new();

    assert!(matches!(
        backend.move_track(invalid_track_id(), 0, invalid_track_id(), 0),
        Err(Error::InvalidId),
    ));

    let root = backend.create_track()?;
    let child1 = backend.create_track()?;
    let child2 = backend.create_track()?;
    let grandchild = backend.create_track()?;
    backend.append_track_child(root, child1)?;
    backend.append_track_child(root, child2)?;
    backend.append_track_child(child1, grandchild)?;

    backend.move_track(root, 0, root, 1)?;

    assert_eq!(backend.get_track_children(root)?, vec![child2, child1]);

    backend.move_track(root, 1, child2, 0)?;

    assert_eq!(backend.get_track_children(root)?, vec![child2]);
    assert_eq!(backend.get_track_children(child2)?, vec![child1]);

    assert!(matches!(
        backend.move_track(root, 0, child2, 0),
        Err(Error::RecursiveTrack)
    ));

    Ok(())
}

#[test]
fn remove_track_child() -> Result<()> {
    let mut backend = Backend::new();

    assert!(matches!(
        backend.remove_track_child(invalid_track_id(), 0),
        Err(Error::InvalidId)
    ));

    let root = backend.create_track()?;
    let child1 = backend.create_track()?;
    let child2 = backend.create_track()?;
    backend.append_track_child(root, child1)?;
    backend.append_track_child(root, child2)?;

    assert!(matches!(
        backend.remove_track_child(child1, 0),
        Err(Error::IndexOutOfBounds)
    ));

    backend.remove_track_child(root, 0)?;
    assert_eq!(backend.get_track_children(root)?, vec![child2]);
    backend.remove_track_child(root, 0)?;
    assert_eq!(backend.get_track_children(root)?, vec![]);

    Ok(())
}

#[test]
#[ignore = "not yet implemented"]
fn move_track_item() -> Result<()> {
    todo!()
}

#[test]
#[ignore = "not yet implemented"]
fn resize_track_item() -> Result<()> {
    todo!()
}

#[test]
#[ignore = "not yet implemented"]
fn get_track_view_item() -> Result<()> {
    todo!()
}
#[test]
#[ignore = "not yet implemented"]
fn get_track_view_range() -> Result<()> {
    todo!()
}
