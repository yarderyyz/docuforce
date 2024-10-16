use crate::cache::{get_cache_entry_by_hash, insert_or_update_cache_entry, CacheEntry};
use crate::types::{DocumentationAssistant, FunctionData};
use async_openai::config::OpenAIConfig;
use async_openai::{
    types::{
        AssistantObject, CreateAssistantRequestArgs, CreateMessageRequestArgs,
        CreateRunRequestArgs, CreateThreadRequestArgs, MessageContent, MessageRole, RunStatus,
    },
    Client,
};
use serde::Deserialize;
use sqlx::{Pool, Sqlite};
use std::error::Error;

const ASSISTANT_NAME: &str = "Naggy";
const DEFAULT_MODEL: &str = "gpt-4o";
const INSTRUCTIONS: &str = r#"
Prompt:

    You are an assistant that checks if the documentation strings match the function definition in Rust code.

Inputs:

    You will be provided with a Rust function definition and its associated documentation.

Task:

    Compare the documentation with the function definition and identify any discrepancies.

Output Instructions:

    Return your response as a raw JSON object, without any additional text or formatting or markdown code blocks.
    The JSON object should contain the following fields:
        "name": The name of the function from the input data.
        "confidence": A float number between 0.0 and 1.0 representing how confident you are that the documentation matches the function. 0.0 means not confident at all, and 1.0 means very confident.
        "hash": An echo of the hash provided.
        "errors": A list of strings detailing errors that must be fixed. Errors include:
            Missing parameters in the documentation.
            Extra parameters in the documentation that are not in the function.
            Incorrect return type in the documentation.
        "warnings": A list of strings containing suggestions to improve the documentation. These should be concise, like compiler or lint warnings. If there are no suggestions, this list can be empty.

Additional Guidelines:

    A point should not be listed in both "errors" and "warnings". If it needs to be fixed, it should be an error; otherwise, it's a warning.
    Ensure that the JSON is valid and includes only the specified fields.
    Do not include any explanatory text outside the JSON object.

Example Output:

{
  "name": "my_function",
  "confidence": 0.9,
  "errors": [
    "Parameter 'threshold' is missing in documentation",
    "Return type in documentation does not match function"
  ],
  "warnings": [
    "Consider adding usage examples to the documentation"
  ]
}
"#;

impl DocumentationAssistant {
    pub async fn create_openai_assistant(
        self: &DocumentationAssistant,
        client: &Client<OpenAIConfig>,
    ) -> Result<AssistantObject, Box<dyn Error>> {
        //create the assistant
        let assistant_request = CreateAssistantRequestArgs::default()
            .name(self.name.clone())
            .instructions(self.instructions.clone())
            .model(self.model.clone())
            .build()?;
        Ok(client.assistants().create(assistant_request).await?)
    }

    pub async fn run_openai_query(
        self: &DocumentationAssistant,
        query_data: Vec<FunctionData<'_>>,
        client: &Client<OpenAIConfig>,
        pool: &Pool<Sqlite>,
    ) -> Result<(), Box<dyn Error>> {
        //create a thread for the conversation
        let thread_request = CreateThreadRequestArgs::default().build()?;
        let thread = client.threads().create(thread_request.clone()).await?;

        let openai_assistant = self.create_openai_assistant(client).await?;

        //get the id of the openai_assistant
        let openai_assistant_id = &openai_assistant.id;

        for data in query_data {
            let entry = get_cache_entry_by_hash(pool, &data.compute_hash()).await?;
            if let Some(entry) = entry {
                println!("Found cached: {:?}", entry);
                continue;
            }

            let input = format!(
                "HASH\n\n{}\nFUNCTION_NAME\n\n{}\nDOC_STRING\n\n{}\nFUNCTION_BODY\n\n{}\n",
                &data.compute_hash(),
                data.name,
                data.doc_string,
                data.body
            );

            //limit the list responses to 1 message
            let query = [("limit", "1")];

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
                .assistant_id(openai_assistant_id)
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
                                panic!("images are not expected");
                            }
                            MessageContent::Refusal(refusal) => refusal.refusal.clone(),
                        };

                        let entry: CacheEntry = serde_json::from_str(&text)?;
                        insert_or_update_cache_entry(pool, &entry).await?;

                        println!("AI Generated: {:?}", entry);
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
        client.assistants().delete(openai_assistant_id).await?;
        client.threads().delete(&thread.id).await?;

        Ok(())
    }
}

impl Default for DocumentationAssistant {
    fn default() -> Self {
        DocumentationAssistant {
            name: ASSISTANT_NAME.to_string(),
            instructions: INSTRUCTIONS.to_string(),
            model: DEFAULT_MODEL.to_string(),
        }
    }
}
