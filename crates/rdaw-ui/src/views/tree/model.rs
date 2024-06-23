use futures::Future;

pub trait TreeModel: 'static {
    type Node: TreeNode;

    fn root(&self) -> Self::Node;

    fn get_children(
        &self,
        parent: &Self::Node,
    ) -> impl Future<Output = Vec<Self::Node>> + Send + 'static;
}

pub trait TreeNode: Send + Clone {
    fn name(&self) -> &str;

    fn has_children(&self) -> bool;
}
