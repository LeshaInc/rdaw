use futures::Future;
use rdaw_core::path::Utf8PathBuf;

use super::{TreeModel, TreeNode};

#[derive(Clone)]
pub struct FsTreeModel {
    root: Utf8PathBuf,
}

impl FsTreeModel {
    pub fn new(root: Utf8PathBuf) -> FsTreeModel {
        FsTreeModel { root }
    }
}

impl TreeModel for FsTreeModel {
    type Node = FsTreeNode;

    fn root(&self) -> FsTreeNode {
        FsTreeNode {
            is_dir: true,
            path: self.root.clone(),
        }
    }

    fn get_children(
        &self,
        parent: &Self::Node,
    ) -> impl Future<Output = Vec<Self::Node>> + Send + 'static {
        let path = parent.path.clone();

        async move {
            let mut children = Vec::new();

            let Ok(entries) = path.read_dir_utf8() else {
                return children;
            };

            for entry in entries {
                let Ok(entry) = entry else { continue };

                let is_dir = entry.file_type().unwrap().is_dir();
                children.push(FsTreeNode {
                    is_dir,
                    path: entry.into_path(),
                })
            }

            children.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name().cmp(b.name())));

            children
        }
    }
}

#[derive(Clone)]
pub struct FsTreeNode {
    pub is_dir: bool,
    pub path: Utf8PathBuf,
}

impl TreeNode for FsTreeNode {
    fn name(&self) -> &str {
        self.path.components().last().unwrap().as_str()
    }

    fn has_children(&self) -> bool {
        self.is_dir
    }
}
