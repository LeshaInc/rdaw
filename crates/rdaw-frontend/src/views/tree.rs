use floem::reactive::{create_effect, RwSignal};
use floem::views::{scroll, virtual_stack, Decorators, VirtualDirection, VirtualItemSize};
use floem::IntoView;
use rdaw_core::collections::ImVec;
use rdaw_core::path::Utf8PathBuf;

pub trait TreeModel: 'static {
    type Node: TreeNode;

    fn get_root(&self) -> Self::Node;

    fn get_children(&self, parent: &Self::Node) -> Vec<Self::Node>;
}

pub trait TreeNode: Clone {
    fn name(&self) -> &str;

    fn has_children(&self) -> bool;
}

#[derive(Clone)]
pub struct FsTreeModel;

impl TreeModel for FsTreeModel {
    type Node = FsTreeNode;

    fn get_root(&self) -> FsTreeNode {
        FsTreeNode {
            is_dir: true,
            path: Utf8PathBuf::from("/"), // FIXME: portability
        }
    }

    fn get_children(&self, parent: &FsTreeNode) -> Vec<FsTreeNode> {
        let mut children = Vec::new();

        let Ok(entries) = parent.path.read_dir_utf8() else {
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

        children
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

pub fn tree<M: TreeModel>(model: M) -> impl IntoView {
    let model = RwSignal::new(model);

    let root = RwSignal::new(Tree {
        node: model.with(|m| m.get_root()),
        children: Vec::new(),
        path: Vec::new(),
    });

    let order = RwSignal::new(ImVec::new());

    create_effect(move |_| {
        order.update(|order| {
            order.clear();

            root.with(|root| {
                root.dfs(&mut |node| {
                    order.push_back((node.node.clone(), node.path.clone()));
                    true
                });
            });
        });
    });

    scroll(virtual_stack(
        VirtualDirection::Vertical,
        VirtualItemSize::Fixed(Box::new(|| 25.0)),
        move || order.get(),
        move |(_, path)| path.clone(),
        move |(node, path)| tree_node(model, root, node, path),
    ))
}

fn tree_node<M: TreeModel>(
    model: RwSignal<M>,
    root: RwSignal<Tree<M::Node>>,
    node: M::Node,
    path: Vec<usize>,
) -> impl IntoView {
    let depth = path.len();

    let name = if node.has_children() {
        format!(" [+] {}", node.name())
    } else {
        node.name().to_string()
    };

    let mut view = name.style(move |s| {
        s.width_full()
            .height(25.0)
            .padding_left(15.0 * (depth as f32))
    });

    if node.has_children() {
        view = view.on_click_stop(move |_| {
            let children = model.with(|m| m.get_children(&node));
            root.update(|root| root.set_children(&path, children));
        });
    }

    view
}

struct Tree<N> {
    node: N,
    path: Vec<usize>,
    children: Vec<Tree<N>>,
}

impl<N> Tree<N> {
    fn set_children(&mut self, path: &[usize], children: Vec<N>) {
        self.set_children_inner(path, 0, children);
    }

    fn set_children_inner(&mut self, path: &[usize], depth: usize, children: Vec<N>) {
        if depth == path.len() {
            self.children = children
                .into_iter()
                .enumerate()
                .map(|(i, node)| {
                    let mut node_path = Vec::with_capacity(path.len() + 1);
                    node_path.extend_from_slice(path);
                    node_path.push(i);
                    Tree {
                        node,
                        path: node_path,
                        children: Vec::new(),
                    }
                })
                .collect();
            return;
        }

        let child = &mut self.children[path[depth]];
        child.set_children_inner(path, depth + 1, children);
    }

    fn dfs(&self, callback: &mut impl FnMut(&Tree<N>) -> bool) {
        if !callback(self) {
            return;
        }

        for child in &self.children {
            child.dfs(callback);
        }
    }
}
