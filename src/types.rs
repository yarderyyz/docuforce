use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct FunctionData<'a> {
    pub name: &'a str,
    pub doc_string: Cow<'a, str>,
    pub body: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DocumentationAssistant {
    pub name: String,
    pub instructions: String,
    pub model: String,
}
