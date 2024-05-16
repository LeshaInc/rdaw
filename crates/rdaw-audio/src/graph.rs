use std::cell::UnsafeCell;
use std::collections::{HashMap, HashSet, VecDeque};

use bumpalo::Bump;
use slotmap::SlotMap;
use smallvec::SmallVec;

use crate::buffer::AudioBuffer;

#[derive(Debug, Clone, Copy)]
pub struct GraphParams {
    pub sample_rate: u32,
    pub buffer_size: usize,
}

slotmap::new_key_type! {
    pub struct NodeId;
}

pub trait Node: Send + Sync + 'static {
    fn num_audio_inputs(&self) -> usize;

    fn num_audio_outputs(&self) -> usize;

    fn compile(&self, params: &GraphParams) -> Box<dyn CompiledNode>;
}

pub trait CompiledNode: Send + 'static {
    fn process(&mut self, params: &GraphParams, inputs: Inputs<'_>, outputs: Outputs<'_>);
}

#[derive(Debug)]
pub struct Inputs<'a> {
    pub audio: &'a [&'a AudioBuffer],
}

#[derive(Debug)]
pub struct Outputs<'a> {
    pub audio: &'a mut [&'a mut AudioBuffer],
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Port {
    Audio(usize),
}

struct NodeEntry {
    node: Box<dyn Node>,
    deps: HashSet<NodeId>,
    rev_deps: HashSet<NodeId>,
    audio_inputs: Vec<Option<(NodeId, usize)>>,
    audio_outputs: Vec<Vec<(NodeId, usize)>>,
}

pub struct Graph {
    params: GraphParams,
    nodes: SlotMap<NodeId, NodeEntry>,
}

impl Graph {
    pub fn new(params: GraphParams) -> Graph {
        Graph {
            params,
            nodes: SlotMap::default(),
        }
    }

    fn toposort(&self) -> Vec<NodeId> {
        let mut indegrees = self
            .nodes
            .iter()
            .map(|(id, entry)| (id, entry.deps.len()))
            .collect::<HashMap<NodeId, usize>>();

        let mut queue = indegrees
            .iter()
            .filter(|&(_, &indegree)| indegree == 0)
            .map(|(&id, _)| id)
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

    pub fn set_params(&mut self, params: GraphParams) {
        self.params = params;
    }

    pub fn add_node<N: Node>(&mut self, node: N) -> NodeId {
        self.nodes.insert(NodeEntry {
            deps: HashSet::new(),
            rev_deps: HashSet::new(),
            audio_inputs: vec![None; node.num_audio_inputs()],
            audio_outputs: vec![vec![]; node.num_audio_outputs()],

            node: Box::new(node),
        })
    }

    pub fn get_node(&self, id: NodeId) -> Option<&dyn Node> {
        self.nodes.get(id.into()).map(|v| &*v.node)
    }

    pub fn remove_node(&mut self, id: NodeId) {
        self.nodes.remove(id.into());
    }

    pub fn connect(
        &mut self,
        (src_node, src_port): (NodeId, Port),
        (dst_node, dst_port): (NodeId, Port),
    ) {
        match (src_port, dst_port) {
            (Port::Audio(src_port), Port::Audio(dst_port)) => {
                self.nodes[src_node].audio_outputs[src_port].push((dst_node, dst_port));
                self.nodes[dst_node].audio_inputs[dst_port] = Some((src_node, src_port))
            }
        }

        self.nodes[dst_node].deps.insert(src_node);
        self.nodes[src_node].rev_deps.insert(dst_node);
    }

    pub fn compile(&self) -> CompiledGraph {
        let mut num_buffers = 1;
        let mut out_buffers = HashMap::with_capacity(self.nodes.len());
        let mut nodes = Vec::with_capacity(self.nodes.len());

        for node_id in self.toposort() {
            let node = &self.nodes[node_id];

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

                out_buffers.insert((node_id, idx), buffer_idx);
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

        CompiledGraph {
            state: State {
                params: self.params,
                bump,
                audio_buffers,
            },
            nodes,
        }
    }
}

struct State {
    params: GraphParams,
    bump: Bump,
    audio_buffers: Vec<UnsafeCell<AudioBuffer>>,
}

pub struct CompiledGraph {
    state: State,
    nodes: Vec<CompiledNodeEntry>,
}

impl CompiledGraph {
    pub fn process(&mut self) {
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
