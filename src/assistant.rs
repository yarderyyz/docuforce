use async_openai::config::OpenAIConfig;
use std::error::Error;

use async_openai::{
    types::{AssistantObject, CreateAssistantRequestArgs},
    Client,
};

const ASSISTANT_NAME: &str = "Naggy";
const DEFAULT_MODEL: &str = "gpt-4o";
const INSTRUCTIONS: &str = "
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
";

use crate::types::DocumentationAssistant;

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
