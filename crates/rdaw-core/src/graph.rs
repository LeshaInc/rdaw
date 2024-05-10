use crate::buffer::AudioBuffer;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct NodeId(pub u64);

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Port {
    Audio(usize),
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

pub trait Graph: Send + Sync + 'static {
    fn set_params(&mut self, params: GraphParams);

    fn add_node<N: Node>(&mut self, node: N) -> NodeId;

    fn get_node(&self, id: NodeId) -> Option<&dyn Node>;

    fn remove_node(&mut self, id: NodeId);

    fn connect(&mut self, src: (NodeId, Port), dst: (NodeId, Port));

    fn compile(&self) -> Box<dyn CompiledGraph>;
}

pub trait CompiledGraph: Send + 'static {
    fn process(&mut self);
}

#[derive(Debug, Clone, Copy)]
pub struct GraphParams {
    pub sample_rate: u32,
    pub buffer_size: usize,
}
