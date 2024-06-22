use futures::StreamExt;
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rdaw_api::document::DocumentOperations;
use rdaw_api::track::{TrackHierarchyEvent, TrackNode, TrackOperations};
use rdaw_api::{assert_err, ErrorKind, Result};

use crate::tests::{invalid_track_id, run_test};

#[test]
fn subscribe_track_name() -> Result<()> {
    run_test(|client| async move {
        let document_id = client.create_document().await?;

        assert_err!(
            client.subscribe_track_name(invalid_track_id()).await,
            ErrorKind::InvalidId,
        );

        let track = client.create_track(document_id).await?;
        let mut stream = client.subscribe_track_name(track).await?;

        client.set_track_name(track, "New name".into()).await?;

        assert_eq!(stream.next().await, Some("New name".into()));

        Ok(())
    })
}

#[test]
fn subscribe_track_hierarchy() -> Result<()> {
    run_test(|client| async move {
        let document_id = client.create_document().await?;

        assert_err!(
            client.subscribe_track_hierarchy(invalid_track_id()).await,
            ErrorKind::InvalidId,
        );

        let root = client.create_track(document_id).await?;
        let mut stream = client.subscribe_track_hierarchy(root).await?;

        let child1 = client.create_track(document_id).await?;
        let child2 = client.create_track(document_id).await?;
        client.append_track_child(root, child1).await?;
        client.append_track_child(root, child2).await?;

        let grandchild = client.create_track(document_id).await?;
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
        let document_id = client.create_document().await?;

        assert_err!(
            client.get_track_name(invalid_track_id()).await,
            ErrorKind::InvalidId,
        );

        let track = client.create_track(document_id).await?;
        assert_eq!(client.get_track_name(track).await?, "Track 2");
        client.set_track_name(track, "New name".into()).await?;
        assert_eq!(client.get_track_name(track).await?, "New name");

        Ok(())
    })
}

#[test]
fn get_track_children() -> Result<()> {
    run_test(|client| async move {
        let document_id = client.create_document().await?;

        assert_err!(
            client.get_track_children(invalid_track_id()).await,
            ErrorKind::InvalidId,
        );

        let root = client.create_track(document_id).await?;
        let child1 = client.create_track(document_id).await?;
        let child2 = client.create_track(document_id).await?;
        client.append_track_child(root, child1).await?;
        client.append_track_child(root, child2).await?;
        assert_eq!(client.get_track_children(root).await?, vec![child1, child2]);

        Ok(())
    })
}

#[test]
fn get_track_hierarchy() -> Result<()> {
    run_test(|client| async move {
        let document_id = client.create_document().await?;

        assert_err!(
            client.get_track_hierarchy(invalid_track_id()).await,
            ErrorKind::InvalidId,
        );

        let root = client.create_track(document_id).await?;
        let child1 = client.create_track(document_id).await?;
        let child2 = client.create_track(document_id).await?;
        let grandchild = client.create_track(document_id).await?;
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
        let document_id = client.create_document().await?;

        assert_err!(
            client
                .append_track_child(invalid_track_id(), invalid_track_id())
                .await,
            ErrorKind::InvalidId,
        );

        let parent = client.create_track(document_id).await?;

        assert_err!(
            client.append_track_child(parent, invalid_track_id()).await,
            ErrorKind::InvalidId,
        );

        let child = client.create_track(document_id).await?;

        assert_err!(
            client.append_track_child(invalid_track_id(), child).await,
            ErrorKind::InvalidId,
        );

        client.append_track_child(parent, child).await?;
        assert_eq!(client.get_track_children(parent).await?, vec![child]);

        Ok(())
    })
}

#[test]
fn insert_track_child() -> Result<()> {
    run_test(|client| async move {
        let document_id = client.create_document().await?;

        assert_err!(
            client
                .insert_track_child(invalid_track_id(), invalid_track_id(), 0)
                .await,
            ErrorKind::InvalidId,
        );

        let parent = client.create_track(document_id).await?;

        assert_err!(
            client
                .insert_track_child(parent, invalid_track_id(), 0)
                .await,
            ErrorKind::InvalidId,
        );

        let child = client.create_track(document_id).await?;

        assert_err!(
            client
                .insert_track_child(invalid_track_id(), child, 0)
                .await,
            ErrorKind::InvalidId,
        );

        assert_err!(
            client.insert_track_child(parent, child, 1).await,
            ErrorKind::IndexOutOfBounds,
        );

        client.insert_track_child(parent, child, 0).await?;
        assert_eq!(client.get_track_children(parent).await?, vec![child]);

        let child0 = client.create_track(document_id).await?;
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
        let document_id = client.create_document().await?;

        assert_err!(
            client
                .move_track(invalid_track_id(), 0, invalid_track_id(), 0)
                .await,
            ErrorKind::InvalidId,
        );

        let root = client.create_track(document_id).await?;
        let child1 = client.create_track(document_id).await?;
        let child2 = client.create_track(document_id).await?;
        let grandchild = client.create_track(document_id).await?;
        client.append_track_child(root, child1).await?;
        client.append_track_child(root, child2).await?;
        client.append_track_child(child1, grandchild).await?;

        client.move_track(root, 0, root, 1).await?;

        assert_eq!(client.get_track_children(root).await?, vec![child2, child1]);

        client.move_track(root, 1, child2, 0).await?;

        assert_eq!(client.get_track_children(root).await?, vec![child2]);
        assert_eq!(client.get_track_children(child2).await?, vec![child1]);

        assert_err!(
            client.move_track(root, 0, child2, 0).await,
            ErrorKind::NotSupported
        );

        Ok(())
    })
}

#[test]
fn move_track_randomized() -> Result<()> {
    const NUM_TRACKS: usize = 10;
    const NUM_ITERATIONS: usize = 1000;

    run_test(|client| async move {
        let document_id = client.create_document().await?;

        let mut rng = SmallRng::seed_from_u64(1);
        let mut tracks = Vec::new();

        for _ in 0..NUM_TRACKS {
            tracks.push(client.create_track(document_id).await?);
        }

        let root = tracks[0];
        for &child in &tracks[1..] {
            client.append_track_child(root, child).await?;
        }

        for _ in 0..NUM_ITERATIONS {
            let old_parent_id = *tracks.choose(&mut rng).unwrap();
            let old_parent_children = client.get_track_children(old_parent_id).await?;
            if old_parent_children.is_empty() {
                continue;
            }

            let old_index = rng.gen_range(0..old_parent_children.len());
            let child_id = old_parent_children[old_index];

            let new_parent_id = *tracks.choose(&mut rng).unwrap();
            let new_index = if old_parent_id == new_parent_id {
                if old_parent_children.len() == 1 {
                    continue;
                }

                rng.gen_range(0..old_parent_children.len())
            } else {
                let new_parent_len = client.get_track_children(new_parent_id).await?.len();
                rng.gen_range(0..=new_parent_len)
            };

            let mut is_recursive = false;
            let hierarchy = client.get_track_hierarchy(child_id).await?;
            hierarchy.dfs(child_id, |node| {
                is_recursive |= node.id == new_parent_id;
            });

            eprintln!(
                "Move {child_id:?} from {old_parent_id:?}:{old_index} to {new_parent_id:?}:{new_index}"
            );

            let res = client
                .move_track(old_parent_id, old_index, new_parent_id, new_index)
                .await;

            if is_recursive {
                assert_err!(res, ErrorKind::NotSupported);
            } else {
                res?;
            }
        }

        Ok(())
    })
}

#[test]
fn remove_track_child() -> Result<()> {
    run_test(|client| async move {
        let document_id = client.create_document().await?;

        assert_err!(
            client.remove_track_child(invalid_track_id(), 0).await,
            ErrorKind::InvalidId
        );

        let root = client.create_track(document_id).await?;
        let child1 = client.create_track(document_id).await?;
        let child2 = client.create_track(document_id).await?;
        client.append_track_child(root, child1).await?;
        client.append_track_child(root, child2).await?;

        assert_err!(
            client.remove_track_child(child1, 0).await,
            ErrorKind::IndexOutOfBounds
        );

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
