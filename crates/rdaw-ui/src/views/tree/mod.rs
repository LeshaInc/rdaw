mod fs;
mod model;

use floem::reactive::{create_effect, RwSignal};
use floem::views::{label, virtual_stack, Decorators, VirtualDirection, VirtualItemSize};
use floem::IntoView;
use rdaw_core::collections::ImVec;

pub use self::fs::{FsTreeModel, FsTreeNode};
pub use self::model::{TreeModel, TreeNode};
use crate::task::spawn;
use crate::theme::Theme;

pub fn tree<M: TreeModel>(model: M) -> impl IntoView {
    let model = RwSignal::new(model);

    let root = RwSignal::new(Tree {
        node: model.with(|m| m.root()),
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

    virtual_stack(
        VirtualDirection::Vertical,
        VirtualItemSize::Fixed(Box::new(|| {
            let theme = Theme::get();
            f64::from(theme.fonts.normal.m.size * 1.5)
        })),
        move || order.get(),
        move |(_, path)| path.clone(),
        move |(node, path)| tree_node(model, root, node, path),
    )
}

fn tree_node<M: TreeModel>(
    model: RwSignal<M>,
    root: RwSignal<Tree<M::Node>>,
    node: M::Node,
    path: Vec<usize>,
) -> impl IntoView {
    let depth = path.len();
    let is_expanded = RwSignal::new(false);
    let has_children = node.has_children();
    let node_name = node.name().to_owned();

    let mut view = label(move || {
        if has_children {
            if is_expanded.get() {
                format!("[−] {}", node_name)
            } else {
                format!("[+] {}", node_name)
            }
        } else {
            node_name.clone()
        }
    })
    .style(move |s| {
        let theme = Theme::get();
        s.width_full()
            .height(theme.fonts.normal.m.size * 1.5)
            .font_family(theme.fonts.normal.m.family.clone())
            .font_size(theme.fonts.normal.m.size)
            .padding_left(15.0 * (depth as f32))
            .padding_right(15.0)
    });

    if node.has_children() {
        view = view.on_click_stop(move |_| {
            is_expanded.update(|v| *v = !*v);

            if is_expanded.get() {
                let node = node.clone();
                let path = path.clone();
                model.with(move |model| {
                    spawn(model.get_children(&node), move |children| {
                        root.update(|root| root.set_children(&path, children));
                    })
                });
            } else {
                root.update(|root| root.set_children(&path, Vec::new()));
            }
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
