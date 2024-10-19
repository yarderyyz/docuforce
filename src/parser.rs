use std::borrow::Cow;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Parser as TreeParser, Query, QueryCursor};

use crate::types::FunctionData;

enum Command {
    CodeBody,
    CodeIdentifier,
    Comment,
}
const COMMANDS: [Command; 3] = [Command::CodeBody, Command::CodeIdentifier, Command::Comment];

impl Command {
    fn as_str(&self) -> &'static str {
        match self {
            Command::CodeBody => "code.body",
            Command::CodeIdentifier => "code.identifier",
            Command::Comment => "comment",
        }
    }
}

pub struct CommentParser {
    ts_parser: TreeParser,
    query: Query,
}

impl CommentParser {
    pub fn maybe_new_rust_parser() -> Option<Self> {
        let mut ts_parser = TreeParser::new();

        // Try to set up parser
        if let Err(err) = ts_parser.set_language(&tree_sitter_rust::LANGUAGE.into()) {
            eprintln!("{}", err);
            return None;
        }

        // Try to load extraction query
        match Query::new(
            &tree_sitter_rust::LANGUAGE.into(),
            include_str!("queries/rust.scm"),
        ) {
            Ok(query) => {
                // Make sure all commands are present in the query
                let names = query.capture_names();
                if !COMMANDS
                    .into_iter()
                    .all(|name| names.contains(&name.as_str()))
                {
                    return None;
                }
                Some(CommentParser { ts_parser, query })
            }
            Err(err) => {
                eprintln!("{}", err);
                None
            }
        }
    }
}

pub fn extract_function_data<'a>(
    source_code: &'a str,
    parser: &mut CommentParser,
) -> Vec<FunctionData<'a>> {
    let tree = parser.ts_parser.parse(source_code, None).unwrap();

    let mut query_cursor = QueryCursor::new();

    let mut matches = query_cursor.matches(&parser.query, tree.root_node(), source_code.as_bytes());

    let comment_index = parser
        .query
        .capture_index_for_name(Command::Comment.as_str())
        .expect(
            "Command::Comment should be guarenteed to exist as part of CommentParser construction",
        );
    let identifier_index = parser
        .query
        .capture_index_for_name(Command::CodeIdentifier.as_str())
        .expect(
            "Command::CodeIdentifier should be guarenteed to exist as part of CommentParser construction"
        );
    let body_index = parser
        .query
        .capture_index_for_name(Command::CodeBody.as_str())
        .expect(
            "Command::CodeBody should be guarenteed to exist as part of CommentParser construction",
        );

    let mut function_data: Vec<FunctionData> = Vec::new();
    while let Some(item) = matches.next() {
        let identifier = item
            .captures
            .iter()
            .find(|capture| capture.index == identifier_index)
            .unwrap();

        let body = item
            .captures
            .iter()
            .find(|capture| capture.index == body_index)
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

        let name = &source_code[identifier.node.byte_range()];
        let body = &source_code[body.node.byte_range()];
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
