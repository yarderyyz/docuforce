use async_openai::Client;
use clap::Parser;
use futures::future::join_all;
use parser::CommentParser;
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::error::Error;

use std::fs;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

mod assistant;
mod cache;
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
    file: Option<String>,
    #[arg(short, long)]
    init: bool,
}

const HIDDEN_DIR: &str = ".docuforce/";
const DEFAULT_DATABSE: &str = ".docuforce/sqlite:cache.db";

async fn init_database() -> Result<(), Box<dyn Error>> {
    let path = Path::new(HIDDEN_DIR);
    fs::create_dir_all(path)?;

    let options = SqliteConnectOptions::new()
        .filename(DEFAULT_DATABSE)
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(options).await?;
    sqlx::query(include_str!("migrations/cache_1.sql"))
        .execute(&pool)
        .await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    unsafe {
        std::env::set_var("RUST_LOG", "ERROR");
    }

    let args = Args::parse();

    if args.init {
        init_database().await?;
    }

    let file = if let Some(file) = &args.file {
        file
    } else {
        return Ok(());
    };

    let options = SqliteConnectOptions::new().filename(DEFAULT_DATABSE);
    let pool = SqlitePool::connect_with(options).await?;

    let source_code = read_file(file)?;

    // Setup tracing subscriber so that library can log the errors
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    //create a client
    let ai_client = Client::new();

    // TODO: Refactor to use DI for openai bs so we can swap clients down the road
    let assistant = types::DocumentationAssistant::default();

    if let Some(mut comment_parser) = CommentParser::maybe_new_rust_parser() {
        let fn_map = parser::extract_function_data(&source_code, &mut comment_parser);
        let ai_queries: Vec<_> = fn_map
            .into_iter()
            .map(|data| assistant.run_openai_query(data, &ai_client, &pool))
            .collect();
        join_all(ai_queries).await;
    } else {
        println!("Failed to load parser")
    }
    Ok(())
}
