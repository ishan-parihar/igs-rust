use crate::types::*;
use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::Path;

/// Initialize the SQLite database schema
pub fn init_db(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS articles (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            pub_date TEXT NOT NULL,
            source_name TEXT NOT NULL,
            domains_json TEXT NOT NULL DEFAULT '[]',
            entities_json TEXT NOT NULL DEFAULT '[]'
        );

        CREATE INDEX IF NOT EXISTS idx_articles_pub_date ON articles(pub_date);
        CREATE INDEX IF NOT EXISTS idx_articles_source ON articles(source_name);
    ",
    )
    .context("Failed to create database schema")?;
    Ok(())
}

/// Save an article to the database
pub fn save_article(conn: &Connection, article: &ArticleInsight) -> Result<()> {
    let domains_json = serde_json::to_string(&article.domains).unwrap_or_else(|_| "[]".to_string());
    let entities_json =
        serde_json::to_string(&article.entities).unwrap_or_else(|_| "[]".to_string());

    conn.execute(
        "INSERT OR REPLACE INTO articles (id, title, pub_date, source_name, domains_json, entities_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![article.id, article.title, article.pub_date, article.source_name, domains_json, entities_json],
    ).context("Failed to save article")?;

    Ok(())
}

/// Load all articles from the database
pub fn load_articles(conn: &Connection) -> Result<Vec<ArticleInsight>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, pub_date, source_name, domains_json, entities_json FROM articles ORDER BY pub_date DESC"
    ).context("Failed to prepare load query")?;

    let articles = stmt
        .query_map([], |row| {
            let domains_json: String = row.get(4)?;
            let entities_json: String = row.get(5)?;

            let domains: Vec<DomainInfo> = serde_json::from_str(&domains_json).unwrap_or_default();
            let entities: Vec<EntityInfo> =
                serde_json::from_str(&entities_json).unwrap_or_default();

            Ok(ArticleInsight {
                id: row.get(0)?,
                title: row.get(1)?,
                pub_date: row.get(2)?,
                source_name: row.get(3)?,
                domains,
                entities,
            })
        })
        .context("Failed to query articles")?;

    let mut result = Vec::new();
    for article in articles {
        result.push(article?);
    }

    Ok(result)
}

/// Get article count
pub fn article_count(conn: &Connection) -> Result<usize> {
    let count: usize = conn
        .query_row("SELECT COUNT(*) FROM articles", [], |row| row.get(0))
        .context("Failed to count articles")?;
    Ok(count)
}

/// Clear all articles
pub fn clear_articles(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM articles", [])
        .context("Failed to clear articles")?;
    Ok(())
}

/// Open or create the database at the given path
pub fn open_db(path: &Path) -> Result<Connection> {
    let conn =
        Connection::open(path).context(format!("Failed to open database at {}", path.display()))?;
    init_db(&conn)?;
    Ok(conn)
}

/// Get the default database path
pub fn default_db_path() -> std::path::PathBuf {
    crate::config::user_config_dir().join("insights.db")
}
