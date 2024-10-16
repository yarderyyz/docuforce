use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use sqlx::FromRow;

use sqlx::SqlitePool;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct CacheEntry {
    name: String,
    confidence: f64,
    hash: String,
    errors: Json<Vec<String>>,
    warnings: Json<Vec<String>>,
}

pub async fn get_cache_entry_by_hash(
    pool: &SqlitePool,
    hash: &str,
) -> Result<Option<CacheEntry>, sqlx::Error> {
    let result = sqlx::query_as::<_, CacheEntry>(
        "SELECT name, confidence, hash, errors, warnings FROM cache WHERE hash = ?",
    )
    .bind(hash)
    .fetch_optional(pool)
    .await?;

    Ok(result)
}

pub async fn insert_or_update_cache_entry(
    pool: &SqlitePool,
    entry: &CacheEntry,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT OR REPLACE INTO cache (name, confidence, hash, errors, warnings)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&entry.name)
    .bind(entry.confidence)
    .bind(&entry.hash)
    .bind(&entry.errors)
    .bind(&entry.warnings)
    .execute(pool)
    .await?;

    Ok(())
}
