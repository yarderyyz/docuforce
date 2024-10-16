use std::borrow::Cow;

use sha2::{Digest, Sha256};

use tree_sitter::Point;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct FunctionData<'a> {
    pub position: Point,
    pub name: &'a str,
    pub doc_string: Cow<'a, str>,
    pub body: &'a str,
}

impl FunctionData<'_> {
    pub fn compute_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(&*self.doc_string);
        hasher.update(self.body);
        let result = hasher.finalize();
        hex::encode(result)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DocumentationAssistant {
    pub name: String,
    pub instructions: String,
    pub model: String,
}
