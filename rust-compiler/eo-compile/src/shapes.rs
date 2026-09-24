//! +-----------------------------------------------------------------+
//! | shapes — interns hidden-class layouts produced by `infer`.       |
//! |                                                                  |
//! | Each distinct (atom, slot-layout) pair becomes a `Shape`. The    |
//! | codegen emits one `static CLASS_<id>: eo_rt::Class` per shape    |
//! | plus the corresponding dispatch fn.                              |
//! +-----------------------------------------------------------------+

#[derive(Debug, Eq, PartialEq, Hash)]
pub struct Shape {
    pub atom: Option<u32>,
    pub slots: Vec<u32>,
}

pub struct ShapeTable {
    pub shapes: Vec<Shape>,
}

impl ShapeTable {
    pub fn new() -> Self {
        Self { shapes: Vec::new() }
    }
}

impl Default for ShapeTable {
    fn default() -> Self {
        Self::new()
    }
}
