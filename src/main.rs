use async_openai::{
    types::{
        CreateAssistantRequestArgs, CreateMessageRequestArgs, CreateRunRequestArgs,
        CreateThreadRequestArgs, MessageContent, MessageRole, RunStatus,
    },
    Client,
};
use std::error::Error;

use std::borrow::Cow;
use std::collections::HashMap;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Parser, Query, QueryCursor};

use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[derive(Debug)]
struct FunctionData<'a> {
    name: &'a str,
    doc_string: Cow<'a, str>,
    body: &'a str,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    unsafe {
        std::env::set_var("RUST_LOG", "ERROR");
    }

    // Setup tracing subscriber so that library can log the errors
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let query = [("limit", "1")]; //limit the list responses to 1 message

    //create a client
    let client = Client::new();

    //create a thread for the conversation
    let thread_request = CreateThreadRequestArgs::default().build()?;
    let thread = client.threads().create(thread_request.clone()).await?;

    let assistant_name = "Naggy".to_string();

    let instructions = "
        You are an assistant checking if the documentation strings match the function definition.
        You are using the rust programming language.
        Return your response in JSON.
        There should be a field called 'confidence' include a float number from 0-1 representing how confident you are the documentation matches the function.
        0 the documentation is completely off. 1 The documentation is a perfect match.
        There should be another field called 'description' which is a string describing how the documentation differs from the implementation.
        If the implemention matches already the 'description' should just read 'Documentation Okay'.
    "
    .to_string();

    //create the assistant
    let assistant_request = CreateAssistantRequestArgs::default()
        .name(&assistant_name)
        .instructions(&instructions)
        .model("gpt-3.5-turbo-1106")
        .build()?;
    let assistant = client.assistants().create(assistant_request).await?;
    //get the id of the assistant
    let assistant_id = &assistant.id;

    let source_code = "
/// Generates a `RenderGraph` from the provided `World` struct.
///
/// # Parameters
///
/// - `world`: A reference to a `World` struct, which contains the game's state, including the board and entities.
/// - `state`: A state object
///
/// # Returns
///
/// A `RenderGraph` struct that represents the entire scene to be rendered. The root node of the graph corresponds to
/// the game's board, and its children correspond to the entities present in the world.
///
/// # Details
///
/// This function creates a hierarchical `RenderGraph` that represents the current state of the game world. The root
/// node of the graph contains a `RenderItem::Board`, which represents the game board. The root node's children are
/// `RenderNode`s that each contain a `RenderItem::Entity`, representing the entities in the world. Each entity is
/// cloned from the `world` and encapsulated in a `RenderNode` without further children (i.e., `children` is `None`).
///
/// This graph structure allows for organized and flexible rendering of the game world. By constructing the scene
/// as a tree of `RenderNode`s, it becomes easier to traverse and render the scene in a systematic way, ensuring
/// that the board is rendered first, followed by the entities in the correct order.
///
/// This function is typically called before rendering to prepare the data needed to generate a visual representation
/// of the game state.
///
/// # Examples
///
/// ```rust
/// let world = World::new(); // Assume World::new initializes a game world
/// let render_graph = generate_render_graph(&world);
///
/// // `render_graph` can now be passed to a rendering function
/// let area = Rect::new(0, 0, 80, 24); // Example rendering area
/// let glyph_buffer = glypherize_graph(render_graph, area, render_fn);
/// ```
fn generate_render_graph(world: &World) -> RenderGraph {
    let children = world
        .entities
        .iter()
        .map(|ent| RenderNode {
            item: RenderItem::Entity(ent),
            children: None,
        })
        .collect();

    RenderGraph {
        root: RenderNode {
            item: RenderItem::Board(&world.board),
            children: Some(children),
        },
    }
}

/// In the level format we cant tell what tiles are floors and what tiles are empty
/// This function takes a board, and working from the outside of the board in culls
/// any floor tiles with an empty neighbour.
pub fn cull_outer_tiles(board: &mut Array2<Tile>) -> &Array2<Tile> {
    let (height, width) = board.dim();
    // Check first and last column for Floor tiles to cull
    for yi in 0..height {
        if matches!(board[[yi, 0]], Tile::Floor) {
            cull_tiles((yi, 0), board);
        }
        if matches!(board[[yi, width - 1]], Tile::Floor) {
            cull_tiles((yi, width - 1), board);
        }
    }
    // Check first and last row for Floor tiles to cull
    for xi in 0..width {
        if matches!(board[[0, xi]], Tile::Floor) {
            cull_tiles((0, xi), board);
        }
        if matches!(board[[height - 1, xi]], Tile::Floor) {
            cull_tiles((height - 1, xi), board);
        }
    }
    board
}
    ";
    let fn_map = extract_function_data(source_code);

    fn_map.iter().for_each(|(_k, data)| {
        println!("fn name: {}", data.name);
        println!("-- fn doc --");
        println!("{}", data.doc_string);
        println!("-- fn body --");
        println!("{}", data.body);
        println!("------");
    });

    for (_k, data) in fn_map {
        let input = format!(
            "doc string\n\n{}\nfunction body\n\n{}\n",
            data.doc_string, data.body
        );

        //create a message for the thread
        let message = CreateMessageRequestArgs::default()
            .role(MessageRole::User)
            .content(input.clone())
            .build()?;

        //attach message to the thread
        let _message_obj = client
            .threads()
            .messages(&thread.id)
            .create(message)
            .await?;

        //create a run for the thread
        let run_request = CreateRunRequestArgs::default()
            .assistant_id(assistant_id)
            .build()?;
        let run = client
            .threads()
            .runs(&thread.id)
            .create(run_request)
            .await?;

        //wait for the run to complete
        let mut awaiting_response = true;
        while awaiting_response {
            //retrieve the run
            let run = client.threads().runs(&thread.id).retrieve(&run.id).await?;
            //check the status of the run
            match run.status {
                RunStatus::Completed => {
                    awaiting_response = false;
                    // once the run is completed we
                    // get the response from the run
                    // which will be the first message
                    // in the thread

                    //retrieve the response from the run
                    let response = client.threads().messages(&thread.id).list(&query).await?;
                    //get the message id from the response
                    let message_id = response.data.first().unwrap().id.clone();
                    //get the message from the response
                    let message = client
                        .threads()
                        .messages(&thread.id)
                        .retrieve(&message_id)
                        .await?;
                    //get the content from the message
                    let content = message.content.first().unwrap();
                    //get the text from the content
                    let text = match content {
                        MessageContent::Text(text) => text.text.value.clone(),
                        MessageContent::ImageFile(_) | MessageContent::ImageUrl(_) => {
                            panic!("imaged are not expected in this example");
                        }
                        MessageContent::Refusal(refusal) => refusal.refusal.clone(),
                    };
                    //print the text
                    println!("--- Response: {}\n", text);
                }
                RunStatus::Failed => {
                    awaiting_response = false;
                    println!("--- Run Failed: {:#?}", run);
                }
                RunStatus::Queued => {
                    println!("--- Run Queued");
                }
                RunStatus::Cancelling => {
                    println!("--- Run Cancelling");
                }
                RunStatus::Cancelled => {
                    println!("--- Run Cancelled");
                }
                RunStatus::Expired => {
                    println!("--- Run Expired");
                }
                RunStatus::RequiresAction => {
                    println!("--- Run Requires Action");
                }
                RunStatus::InProgress => {
                    println!("--- In Progress ...");
                }
                RunStatus::Incomplete => {
                    println!("--- Run Incomplete");
                }
            }
            //wait for 1 second before checking the status again
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }

    //once we have broken from the main loop we can delete the assistant and thread
    client.assistants().delete(assistant_id).await?;
    client.threads().delete(&thread.id).await?;

    Ok(())
}

fn extract_function_data(source_code: &str) -> HashMap<&str, FunctionData> {
    let mut parser = Parser::new();
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

        let comments: Vec<&str> = item
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
    fn_map
}
