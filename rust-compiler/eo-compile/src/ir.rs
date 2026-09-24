//! +-----------------------------------------------------------------+
//! | ir — typed graph that survives between parse and codegen.       |
//! |                                                                  |
//! | One `Node` per DSL statement; ids are dense (0..N) and the       |
//! | parser guarantees they match line position, so `NodeId(i)` is    |
//! | the i-th line. Edges to other nodes are `NodeId`, edges to the   |
//! | "context" (ξ) are encoded as `Target::Sentinel`.                 |
//! |                                                                  |
//! | Attribute names start out as strings and stay that way until     |
//! | `infer` / `shapes` interns them to numeric ids. Keeping them as  |
//! | strings at this layer is intentional — the parser does not       |
//! | own an interner and we want the IR to be inspectable.            |
//! +-----------------------------------------------------------------+

#[derive(Debug, Eq, PartialEq, Copy, Clone, Hash)]
pub struct NodeId(pub u32);

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum Target {
    Object(NodeId),
    Sentinel,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum AttrRef {
    Slot(u32),
    Name(String),
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum AttrSpec {
    Void,
    Ref { id: NodeId, cached: bool },
    Delta(Vec<u8>),
    Atom(String),
}

#[derive(Debug)]
pub enum Node {
    Formation {
        name: String,
        attrs: Vec<(String, AttrSpec)>,
    },
    Dispatch {
        target: Target,
        attr: AttrRef,
    },
    Application {
        target: Target,
        attr: AttrRef,
        value: NodeId,
    },
    Context,
}

impl Node {
    pub fn kind(&self) -> NodeKind {
        match self {
            Node::Formation { .. } => NodeKind::Formation,
            Node::Dispatch { .. } => NodeKind::Dispatch,
            Node::Application { .. } => NodeKind::Application,
            Node::Context => NodeKind::Context,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum NodeKind {
    Formation,
    Dispatch,
    Application,
    Context,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum TypeHint {
    Unknown,
    SmallInt,
    Bool,
    Object,
}

#[derive(Debug)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub hints: Vec<TypeHint>,
}

impl Graph {
    pub fn new() -> Self {
        Self { nodes: Vec::new(), hints: Vec::new() }
    }

    pub fn push(&mut self, node: Node) -> NodeId {
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(node);
        self.hints.push(TypeHint::Unknown);
        id
    }

    pub fn count_by_kind(&self) -> [usize; 4] {
        let mut counts = [0usize; 4];
        for node in &self.nodes {
            counts[node.kind() as usize] += 1;
        }
        counts
    }
}

impl Default for Graph {
    fn default() -> Self {
        Self::new()
    }
}
