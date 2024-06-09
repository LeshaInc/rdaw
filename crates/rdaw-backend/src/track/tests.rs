use futures::StreamExt;
use rdaw_api::track::{TrackEvent, TrackHierarchyEvent, TrackNode, TrackOperations};
use rdaw_api::{Error, Result};

use crate::tests::{invalid_track_id, run_test};

#[test]
fn list_tracks() -> Result<()> {
    run_test(|client| async move {
        let tracks = client.list_tracks().await?;
        assert!(tracks.is_empty());

        let track1 = client.create_track().await?;
        let track2 = client.create_track().await?;

        let mut tracks = client.list_tracks().await?;
        tracks.sort_unstable();
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks, vec![track1, track2]);

        Ok(())
    })
}

#[test]
fn create_track() -> Result<()> {
    run_test(|client| async move {
        let track1 = client.create_track().await?;
        let track2 = client.create_track().await?;
        assert!(track1 != track2);

        Ok(())
    })
}

#[test]
fn subscribe_track() -> Result<()> {
    run_test(|client| async move {
        assert!(matches!(
            client.subscribe_track(invalid_track_id()).await,
            Err(Error::InvalidId),
        ));

        let track = client.create_track().await?;
        let mut stream = client.subscribe_track(track).await?;

        client.set_track_name(track, "New name".into()).await?;

        assert_eq!(
            stream.next().await,
            Some(TrackEvent::NameChanged {
                new_name: "New name".into()
            })
        );

        Ok(())
    })
}

#[test]
fn subscribe_track_hierarchy() -> Result<()> {
    run_test(|client| async move {
        assert!(matches!(
            client.subscribe_track_hierarchy(invalid_track_id()).await,
            Err(Error::InvalidId),
        ));

        let root = client.create_track().await?;
        let mut stream = client.subscribe_track_hierarchy(root).await?;

        let child1 = client.create_track().await?;
        let child2 = client.create_track().await?;
        client.append_track_child(root, child1).await?;
        client.append_track_child(root, child2).await?;

        let grandchild = client.create_track().await?;
        client.append_track_child(child1, grandchild).await?;

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

        Ok(())
    })
}

#[test]
#[ignore = "not yet implemented"]
fn subscribe_track_view() -> Result<()> {
    todo!()
}

#[test]
fn get_set_track_name() -> Result<()> {
    run_test(|client| async move {
        assert!(matches!(
            client.get_track_name(invalid_track_id()).await,
            Err(Error::InvalidId),
        ));

        let track = client.create_track().await?;
        assert_eq!(client.get_track_name(track).await?, "Track 1");
        client.set_track_name(track, "New name".into()).await?;
        assert_eq!(client.get_track_name(track).await?, "New name");

        Ok(())
    })
}

#[test]
fn get_track_children() -> Result<()> {
    run_test(|client| async move {
        assert!(matches!(
            client.get_track_children(invalid_track_id()).await,
            Err(Error::InvalidId),
        ));

        let root = client.create_track().await?;
        let child1 = client.create_track().await?;
        let child2 = client.create_track().await?;
        client.append_track_child(root, child1).await?;
        client.append_track_child(root, child2).await?;
        assert_eq!(client.get_track_children(root).await?, vec![child1, child2]);

        Ok(())
    })
}

#[test]
fn get_track_hierarchy() -> Result<()> {
    run_test(|client| async move {
        assert!(matches!(
            client.get_track_hierarchy(invalid_track_id()).await,
            Err(Error::InvalidId),
        ));

        let root = client.create_track().await?;
        let child1 = client.create_track().await?;
        let child2 = client.create_track().await?;
        let grandchild = client.create_track().await?;
        client.append_track_child(root, child1).await?;
        client.append_track_child(root, child2).await?;
        client.append_track_child(child1, grandchild).await?;

        let hierarchy = client.get_track_hierarchy(child1).await?;
        assert_eq!(hierarchy.root(), child1);
        assert_eq!(
            hierarchy.children(child1).collect::<Vec<_>>(),
            vec![grandchild]
        );

        let hierarchy = client.get_track_hierarchy(child2).await?;
        assert_eq!(hierarchy.root(), child2);
        assert_eq!(hierarchy.children(child2).count(), 0);

        let hierarchy = client.get_track_hierarchy(root).await?;
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
    })
}

#[test]
fn append_track_child() -> Result<()> {
    run_test(|client| async move {
        assert!(matches!(
            client
                .append_track_child(invalid_track_id(), invalid_track_id())
                .await,
            Err(Error::InvalidId),
        ));

        let parent = client.create_track().await?;

        assert!(matches!(
            client.append_track_child(parent, invalid_track_id()).await,
            Err(Error::InvalidId),
        ));

        let child = client.create_track().await?;

        assert!(matches!(
            client.append_track_child(invalid_track_id(), child).await,
            Err(Error::InvalidId),
        ));

        client.append_track_child(parent, child).await?;
        assert_eq!(client.get_track_children(parent).await?, vec![child]);

        Ok(())
    })
}

#[test]
fn insert_track_child() -> Result<()> {
    run_test(|client| async move {
        assert!(matches!(
            client
                .insert_track_child(invalid_track_id(), invalid_track_id(), 0)
                .await,
            Err(Error::InvalidId),
        ));

        let parent = client.create_track().await?;

        assert!(matches!(
            client
                .insert_track_child(parent, invalid_track_id(), 0)
                .await,
            Err(Error::InvalidId),
        ));

        let child = client.create_track().await?;

        assert!(matches!(
            client
                .insert_track_child(invalid_track_id(), child, 0)
                .await,
            Err(Error::InvalidId),
        ));

        assert!(matches!(
            client.insert_track_child(parent, child, 1).await,
            Err(Error::IndexOutOfBounds),
        ));

        client.insert_track_child(parent, child, 0).await?;
        assert_eq!(client.get_track_children(parent).await?, vec![child]);

        let child0 = client.create_track().await?;
        client.insert_track_child(parent, child0, 0).await?;
        assert_eq!(
            client.get_track_children(parent).await?,
            vec![child0, child]
        );

        Ok(())
    })
}

#[test]
fn move_track() -> Result<()> {
    run_test(|client| async move {
        assert!(matches!(
            client
                .move_track(invalid_track_id(), 0, invalid_track_id(), 0)
                .await,
            Err(Error::InvalidId),
        ));

        let root = client.create_track().await?;
        let child1 = client.create_track().await?;
        let child2 = client.create_track().await?;
        let grandchild = client.create_track().await?;
        client.append_track_child(root, child1).await?;
        client.append_track_child(root, child2).await?;
        client.append_track_child(child1, grandchild).await?;

        client.move_track(root, 0, root, 1).await?;

        assert_eq!(client.get_track_children(root).await?, vec![child2, child1]);

        client.move_track(root, 1, child2, 0).await?;

        assert_eq!(client.get_track_children(root).await?, vec![child2]);
        assert_eq!(client.get_track_children(child2).await?, vec![child1]);

        assert!(matches!(
            client.move_track(root, 0, child2, 0).await,
            Err(Error::RecursiveTrack)
        ));

        Ok(())
    })
}

#[test]
fn remove_track_child() -> Result<()> {
    run_test(|client| async move {
        assert!(matches!(
            client.remove_track_child(invalid_track_id(), 0).await,
            Err(Error::InvalidId)
        ));

        let root = client.create_track().await?;
        let child1 = client.create_track().await?;
        let child2 = client.create_track().await?;
        client.append_track_child(root, child1).await?;
        client.append_track_child(root, child2).await?;

        assert!(matches!(
            client.remove_track_child(child1, 0).await,
            Err(Error::IndexOutOfBounds)
        ));

        client.remove_track_child(root, 0).await?;
        assert_eq!(client.get_track_children(root).await?, vec![child2]);
        client.remove_track_child(root, 0).await?;
        assert_eq!(client.get_track_children(root).await?, vec![]);

        Ok(())
    })
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
