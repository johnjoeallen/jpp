use std::path::PathBuf;

use jpp_diagnostics::SourceLocation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JppFile {
    pub path: Option<PathBuf>,
    pub source: String,
    pub class_name: Option<String>,
    pub segments: Vec<Segment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    Java(String),
    Property(Property),
    Mapper(Mapper),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    pub name: String,
    pub ty: String,
    pub getter: bool,
    pub setter: bool,
    pub once: bool,
    pub final_field: bool,
    pub getter_body: Option<String>,
    pub setter_body: Option<String>,
    pub location: SourceLocation,
    pub indent: String,
}

impl Property {
    pub fn has_backing_field(&self) -> bool {
        self.setter || self.once || self.final_field || self.getter_body.is_none()
    }

    pub fn is_calculated(&self) -> bool {
        self.getter && !self.setter && !self.once && !self.final_field && self.getter_body.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapper {
    pub target_type: String,
    pub method_name: String,
    pub assignments: Vec<MapperAssignment>,
    pub location: SourceLocation,
    pub indent: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapperAssignment {
    pub target_property: String,
    pub expression: String,
}
