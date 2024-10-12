use async_openai::config::OpenAIConfig;
use async_openai::{
    types::{
        AssistantObject, CreateAssistantRequestArgs, CreateMessageRequestArgs,
        CreateRunRequestArgs, CreateThreadRequestArgs, MessageContent, MessageRole, RunStatus,
    },
    Client,
};
use clap::Parser;
use std::error::Error;

use std::fs::File;
use std::io::{self, Read};

use std::borrow::Cow;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Parser as TreeParser, Query, QueryCursor};

use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

fn read_file(filename: &str) -> Result<String, io::Error> {
    let mut file = File::open(filename)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Name of the person to greet
    #[arg(short, long)]
    file: String,
}

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

    let args = Args::parse();
    let source_code = read_file(&args.file)?;

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

    let assistant = create_assistant(&client).await?;
    //get the id of the assistant
    let assistant_id = &assistant.id;

    let fn_map = extract_function_data(&source_code);

    for data in fn_map {
        let input = format!(
            "FUNCTION_NAME\n\n{}\nDOC_STRING\n\n{}\nFUNCTION_BODY\n\n{}\n",
            data.name, data.doc_string, data.body
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
                    println!("{}\n", text);
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
                    // println!("--- In Progress ...");
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

async fn create_assistant(
    client: &Client<OpenAIConfig>,
) -> Result<AssistantObject, Box<dyn Error>> {
    let assistant_name = "Naggy".to_string();

    let instructions = "
        Prompt:

            You are an assistant that checks if the documentation strings match the function definition in Rust code.

        Inputs:

            You will be provided with a Rust function definition and its associated documentation.

        Task:

            Compare the documentation with the function definition and identify any discrepancies.

        Output Instructions:

            Return your response as a raw JSON object, without any additional text or formatting or markdown code blocks.
            The JSON object should contain the following fields:
                \"name\": The name of the function from the input data.
                \"confidence\": A float number between 0.0 and 1.0 representing how confident you are that the documentation matches the function. 0.0 means not confident at all, and 1.0 means very confident.
                \"errors\": A list of strings detailing errors that must be fixed. Errors include:
                    Missing parameters in the documentation.
                    Extra parameters in the documentation that are not in the function.
                    Incorrect return type in the documentation.
                \"warnings\": A list of strings containing suggestions to improve the documentation. These should be concise, like compiler or lint warnings. If there are no suggestions, this list can be empty.

        Additional Guidelines:

            A point should not be listed in both \"errors\" and \"warnings\". If it needs to be fixed, it should be an error; otherwise, it's a warning.
            Ensure that the JSON is valid and includes only the specified fields.
            Do not include any explanatory text outside the JSON object.

        Example Output:

        {
          \"name\": \"my_function\",
          \"confidence\": 0.9,
          \"errors\": [
            \"Parameter 'threshold' is missing in documentation\",
            \"Return type in documentation does not match function\"
          ],
          \"warnings\": [
            \"Consider adding usage examples to the documentation\"
          ]
        }
    "
    .to_string();

    //create the assistant
    let assistant_request = CreateAssistantRequestArgs::default()
        .name(&assistant_name)
        .instructions(&instructions)
        .model("gpt-4o")
        .build()?;
    Ok(client.assistants().create(assistant_request).await?)
}

fn extract_function_data(source_code: &str) -> Vec<FunctionData> {
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
        function_data.push(FunctionData {
            name,
            body,
            doc_string,
        });
    }
    function_data
}
