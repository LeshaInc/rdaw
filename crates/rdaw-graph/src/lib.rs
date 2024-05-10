use std::cell::UnsafeCell;
use std::collections::{HashMap, HashSet, VecDeque};

use bumpalo::Bump;
use rdaw_core::buffer::AudioBuffer;
use rdaw_core::graph::{
    CompiledGraph, CompiledNode, Graph, GraphParams, Inputs, Node, NodeId, Outputs, Port,
};
use slotmap::{KeyData, SlotMap};
use smallvec::SmallVec;

slotmap::new_key_type! {
    struct NodeKey;
}

impl From<NodeId> for NodeKey {
    fn from(value: NodeId) -> Self {
        NodeKey(KeyData::from_ffi(value.0))
    }
}

impl From<NodeKey> for NodeId {
    fn from(value: NodeKey) -> Self {
        NodeId(value.0.as_ffi())
    }
}

struct NodeEntry {
    node: Box<dyn Node>,
    deps: HashSet<NodeKey>,
    rev_deps: HashSet<NodeKey>,
    audio_inputs: Vec<Option<(NodeKey, usize)>>,
    audio_outputs: Vec<Vec<(NodeKey, usize)>>,
}

pub struct GraphImpl {
    params: GraphParams,
    nodes: SlotMap<NodeKey, NodeEntry>,
}

impl GraphImpl {
    pub fn new(params: GraphParams) -> GraphImpl {
        GraphImpl {
            params,
            nodes: SlotMap::default(),
        }
    }

    fn toposort(&self) -> Vec<NodeKey> {
        let mut indegrees = self
            .nodes
            .iter()
            .map(|(key, entry)| (key, entry.deps.len()))
            .collect::<HashMap<NodeKey, usize>>();

        let mut queue = indegrees
            .iter()
            .filter(|&(_, &indegree)| indegree == 0)
            .map(|(&key, _)| key)
            .collect::<VecDeque<_>>();

        let mut order = Vec::new();

        while let Some(node) = queue.pop_front() {
            order.push(node);
            for neighbor in &self.nodes[node].rev_deps {
                let indegree = indegrees.get_mut(neighbor).unwrap();
                *indegree -= 1;
                if *indegree == 0 {
                    queue.push_back(*neighbor);
                }
            }
        }

        if order.len() != self.nodes.len() {
            panic!("cycle detected"); // TODO
        }

        order
    }
}

impl Graph for GraphImpl {
    fn set_params(&mut self, params: GraphParams) {
        self.params = params;
    }

    fn add_node<N: Node>(&mut self, node: N) -> NodeId {
        let key = self.nodes.insert(NodeEntry {
            deps: HashSet::new(),
            rev_deps: HashSet::new(),
            audio_inputs: vec![None; node.num_audio_inputs()],
            audio_outputs: vec![vec![]; node.num_audio_outputs()],

            node: Box::new(node),
        });

        NodeId(key.0.as_ffi())
    }

    fn get_node(&self, id: NodeId) -> Option<&dyn Node> {
        self.nodes.get(id.into()).map(|v| &*v.node)
    }

    fn remove_node(&mut self, id: NodeId) {
        self.nodes.remove(id.into());
    }

    fn connect(
        &mut self,
        (src_node, src_port): (NodeId, Port),
        (dst_node, dst_port): (NodeId, Port),
    ) {
        let src_node = NodeKey::from(src_node);
        let dst_node = NodeKey::from(dst_node);

        match (src_port, dst_port) {
            (Port::Audio(src_port), Port::Audio(dst_port)) => {
                self.nodes[src_node].audio_outputs[src_port].push((dst_node, dst_port));
                self.nodes[dst_node].audio_inputs[dst_port] = Some((src_node, src_port))
            }
        }

        self.nodes[dst_node].deps.insert(src_node);
        self.nodes[src_node].rev_deps.insert(dst_node);
    }

    fn compile(&self) -> Box<dyn CompiledGraph> {
        let mut num_buffers = 1;
        let mut out_buffers = HashMap::with_capacity(self.nodes.len());
        let mut nodes = Vec::with_capacity(self.nodes.len());

        for node_key in self.toposort() {
            let node = &self.nodes[node_key];

            let audio_inputs = node
                .audio_inputs
                .iter()
                .map(|src| match src {
                    Some(src) => *out_buffers.get(src).unwrap(),
                    None => 0,
                })
                .collect();

            let mut audio_outputs = SmallVec::new();
            for (idx, _dsts) in node.audio_outputs.iter().enumerate() {
                let buffer_idx = num_buffers;
                num_buffers += 1;

                out_buffers.insert((node_key, idx), buffer_idx);
                audio_outputs.push(buffer_idx);
            }

            nodes.push(CompiledNodeEntry {
                node: node.node.compile(&self.params),
                audio_inputs,
                audio_outputs,
            });
        }

        let bump = Bump::with_capacity(1024);
        bump.set_allocation_limit(Some(0));

        let audio_buffers = (0..num_buffers)
            .map(|_| UnsafeCell::new(AudioBuffer::new(self.params.buffer_size)))
            .collect();

        Box::new(CompiledGraphImpl {
            state: State {
                params: self.params,
                bump,
                audio_buffers,
            },
            nodes,
        })
    }
}

struct State {
    params: GraphParams,
    bump: Bump,
    audio_buffers: Vec<UnsafeCell<AudioBuffer>>,
}

pub struct CompiledGraphImpl {
    state: State,
    nodes: Vec<CompiledNodeEntry>,
}

impl CompiledGraph for CompiledGraphImpl {
    fn process(&mut self) {
        self.state.bump.reset();

        for node in &mut self.nodes {
            node.process(&mut self.state);
        }
    }
}

struct CompiledNodeEntry {
    node: Box<dyn CompiledNode>,
    audio_inputs: SmallVec<[usize; 4]>,
    audio_outputs: SmallVec<[usize; 4]>,
}

impl CompiledNodeEntry {
    fn process(&mut self, state: &mut State) {
        let inputs = Inputs {
            audio: state.bump.alloc_slice_fill_iter(
                self.audio_inputs
                    .iter()
                    .map(|&idx| unsafe { &*state.audio_buffers[idx].get() }),
            ),
        };

        let outputs = Outputs {
            audio: state.bump.alloc_slice_fill_iter(
                self.audio_outputs
                    .iter()
                    .map(|&idx| unsafe { &mut *state.audio_buffers[idx].get() }),
            ),
        };

        self.node.process(&state.params, inputs, outputs);
    }
}
