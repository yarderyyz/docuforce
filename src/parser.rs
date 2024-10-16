use std::borrow::Cow;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Parser as TreeParser, Query, QueryCursor};

use crate::types::FunctionData;

pub fn extract_function_data(source_code: &str) -> Vec<FunctionData> {
    let mut parser = TreeParser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .expect("Error loading Rust grammar");

    let comment_query = Query::new(
        &tree_sitter_rust::LANGUAGE.into(),
        "
        ((line_comment
          doc: (doc_comment) @comment)*
          .
          (function_item name: (identifier) @function_name) @function_def)
        ",
    )
    .unwrap();

    let tree = parser.parse(source_code, None).unwrap();

    let mut query_cursor = QueryCursor::new();

    let mut matches =
        query_cursor.matches(&comment_query, tree.root_node(), source_code.as_bytes());

    let comment_index = comment_query.capture_index_for_name("comment").unwrap();
    let function_name_index = comment_query
        .capture_index_for_name("function_name")
        .unwrap();
    let function_def_index = comment_query
        .capture_index_for_name("function_def")
        .unwrap();

    let mut function_data: Vec<FunctionData> = Vec::new();
    while let Some(item) = matches.next() {
        let function_name = item
            .captures
            .iter()
            .find(|capture| capture.index == function_name_index)
            .unwrap();

        let function_body = item
            .captures
            .iter()
            .find(|capture| capture.index == function_def_index)
            .unwrap();

        let comment_nodes: Vec<_> = item
            .captures
            .iter()
            .filter_map(|capture| {
                if capture.index == comment_index {
                    Some(&capture.node)
                } else {
                    None
                }
            })
            .collect();

        let position = comment_nodes
            .first()
            .map(|node| node.start_position())
            .unwrap_or_default();

        let comments: Vec<&str> = comment_nodes
            .into_iter()
            .map(|node| &source_code[node.byte_range()])
            .collect();

        let name = &source_code[function_name.node.byte_range()];
        let body = &source_code[function_body.node.byte_range()];
        let doc_string = if comments.len() == 1 {
            Cow::Borrowed(comments[0])
        } else {
            Cow::Owned(comments.concat())
        };
        function_data.push(FunctionData {
            position,
            name,
            body,
            doc_string,
        });
    }
    function_data
}
