use async_openai::Client;
use clap::Parser;
use std::error::Error;

use std::fs::File;
use std::io::{self, Read};

mod assistant;
mod parser;
mod types;

use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

fn read_file(filename: &str) -> Result<String, io::Error> {
    let mut file = File::open(filename)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    file: String,
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

    //create a client
    let client = Client::new();

    // TODO: Refactor to use DI for openai bs so we can swap clients down the road
    let assistant = types::DocumentationAssistant::default();

    let fn_map = parser::extract_function_data(&source_code);

    // TODO: build sqlite3 cache and only run query for functions that have changed since last run.
    assistant.run_openai_query(fn_map, &client).await?;

    // TODO: after query runs update database
    Ok(())
}
