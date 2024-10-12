use std::borrow::Cow;
use std::collections::HashMap;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Parser, Query, QueryCursor};

#[derive(Debug)]
struct FunctionData<'a> {
    name: &'a str,
    doc_string: Cow<'a, str>,
    body: &'a str,
}

fn main() {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .expect("Error loading Rust grammar");

    let source_code = "
/// fn test DOC Comment 1
/// fn test DOC Comment 2
fn test() {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .expect(\"Error loading Rust grammar\");
    /// fn test Inner function doc comment
}

/// another_one DOC Comment
/// another_one DOC Comment
fn another_one() {
    /// another_one Inner function doc comment
    let tree = parser.parse(source_code, None).unwrap();
    let root_node = tree.root_node();
}
    ";

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
    let root_node = tree.root_node();
    println!("{}", root_node);

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

    let mut fn_map: HashMap<&str, FunctionData> = HashMap::new();
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

        let mut comments: Vec<&str> = item
            .captures
            .iter()
            .filter_map(|capture| {
                if capture.index == comment_index {
                    Some(&source_code[capture.node.byte_range()])
                } else {
                    None
                }
            })
            .collect();

        let name = &source_code[function_name.node.byte_range()];
        let body = &source_code[function_body.node.byte_range()];
        let doc_string = if comments.len() == 1 {
            Cow::Borrowed(comments[0])
        } else {
            Cow::Owned(comments.concat())
        };
        fn_map.insert(
            name,
            FunctionData {
                name,
                body,
                doc_string,
            },
        );
    }
    fn_map.iter().for_each(|(_k, data)| {
        println!("fn name: {}", data.name);
        println!("-- fn doc --");
        println!("{}", data.doc_string);
        println!("-- fn body --");
        println!("{}", data.body);
        println!("------");
    });
}
