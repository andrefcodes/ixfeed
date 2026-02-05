//! SQLite database management for storing submitted URLs

/// Copyright (C) 2026 Andre Franca <andre@abf.li>
///
/// This program is free software: you can redistribute it and/or modify
/// it under the terms of the GNU Affero General Public License as published by
/// the Free Software Foundation, either version 3 of the License, or
/// (at your option) any later version.
///
/// This program is distributed in the hope that it will be useful,
/// but WITHOUT ANY WARRANTY; without even the implied warranty of
/// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
/// GNU Affero General Public License for more details.
///
/// You should have received a copy of the GNU Affero General Public License
/// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use colored::*;
use dialoguer::Confirm;
use rusqlite::{Connection, OptionalExtension, Result as SqlResult};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub fn db_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let data_dir = dirs::data_dir()
        .ok_or("Could not determine data directory")?
        .join("ixfeed");
    fs::create_dir_all(&data_dir)?;
    Ok(data_dir.join("ixfeed.db"))
}

pub fn init_db() -> Result<Connection, Box<dyn std::error::Error>> {
    let path = db_path()?;
    let conn = Connection::open(&path)?;

    // Sources table for multiple feeds/sitemaps with per-source config
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sources (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_type TEXT NOT NULL,
            source_url TEXT UNIQUE NOT NULL,
            api_key TEXT NOT NULL DEFAULT '',
            host TEXT NOT NULL DEFAULT '',
            searchengine TEXT NOT NULL DEFAULT 'api.indexnow.org',
            first_run_completed INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        )",
        [],
    )?;

    // URLs table with last_modified tracking and source association
    conn.execute(
        "CREATE TABLE IF NOT EXISTS submitted_urls (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_id INTEGER,
            url TEXT NOT NULL,
            last_modified TEXT,
            submitted_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
            UNIQUE(source_id, url),
            FOREIGN KEY (source_id) REFERENCES sources(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Config table (for legacy/global settings)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS config (
            key TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL
        )",
        [],
    )?;

    // App state table (for global flags, etc.)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS app_state (
            key TEXT PRIMARY KEY NOT NULL,
            value INTEGER NOT NULL
        )",
        [],
    )?;

    // Migration: add source_id column to submitted_urls if it doesn't exist
    let has_source_id: bool = conn
        .prepare("SELECT source_id FROM submitted_urls LIMIT 1")
        .is_ok();
    if !has_source_id {
        let _ = conn.execute(
            "ALTER TABLE submitted_urls ADD COLUMN source_id INTEGER",
            [],
        );
    }

    // Migration: add last_modified column if it doesn't exist
    let has_last_modified: bool = conn
        .prepare("SELECT last_modified FROM submitted_urls LIMIT 1")
        .is_ok();
    if !has_last_modified {
        let _ = conn.execute(
            "ALTER TABLE submitted_urls ADD COLUMN last_modified TEXT",
            [],
        );
    }

    // Migration: ensure UNIQUE(source_id, url) constraint exists
    // SQLite doesn't allow adding constraints via ALTER TABLE, so we need to check
    // if the constraint exists and recreate the table if it doesn't
    let has_unique_constraint: bool = {
        let sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name='submitted_urls'",
                [],
                |row| row.get(0),
            )
            .unwrap_or_default();
        sql.contains("UNIQUE(source_id, url)")
    };
    if !has_unique_constraint {
        // Recreate the table with the proper unique constraint
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS submitted_urls_new (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_id INTEGER,
                url TEXT NOT NULL,
                last_modified TEXT,
                submitted_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                UNIQUE(source_id, url),
                FOREIGN KEY (source_id) REFERENCES sources(id) ON DELETE CASCADE
            );
            INSERT OR IGNORE INTO submitted_urls_new (id, source_id, url, last_modified, submitted_at)
                SELECT id, source_id, url, last_modified, submitted_at FROM submitted_urls;
            DROP TABLE submitted_urls;
            ALTER TABLE submitted_urls_new RENAME TO submitted_urls;"
        )?;
    }

    // Migration: add per-source config columns if they don't exist
    let has_api_key: bool = conn
        .prepare("SELECT api_key FROM sources LIMIT 1")
        .is_ok();
    if !has_api_key {
        let _ = conn.execute("ALTER TABLE sources ADD COLUMN api_key TEXT NOT NULL DEFAULT ''", []);
        let _ = conn.execute("ALTER TABLE sources ADD COLUMN host TEXT NOT NULL DEFAULT ''", []);
        let _ = conn.execute("ALTER TABLE sources ADD COLUMN searchengine TEXT NOT NULL DEFAULT 'api.indexnow.org'", []);
    }

    // Migration: migrate old single-source config to sources table
    migrate_legacy_source(&conn)?;

    Ok(conn)
}

/// Migrate legacy single-source config to the new sources table
fn migrate_legacy_source(conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
    // Check if we have legacy config
    let legacy_url: Option<String> = conn
        .query_row("SELECT value FROM config WHERE key = 'source_url'", [], |row| row.get(0))
        .ok();
    
    if let Some(url) = legacy_url {
        // Check if this source already exists in sources table
        let exists: bool = conn.query_row(
            "SELECT 1 FROM sources WHERE source_url = ?1",
            [&url],
            |_| Ok(true),
        ).unwrap_or(false);
        
        if !exists && !url.is_empty() {
            let source_type: String = conn
                .query_row("SELECT value FROM config WHERE key = 'source_type'", [], |row| row.get(0))
                .unwrap_or_else(|_| "feed".to_string());
            
            // Get legacy API settings
            let api_key: String = conn
                .query_row("SELECT value FROM config WHERE key = 'api_key'", [], |row| row.get(0))
                .unwrap_or_default();
            let host: String = conn
                .query_row("SELECT value FROM config WHERE key = 'host'", [], |row| row.get(0))
                .unwrap_or_default();
            let searchengine: String = conn
                .query_row("SELECT value FROM config WHERE key = 'searchengine'", [], |row| row.get(0))
                .unwrap_or_else(|_| "api.indexnow.org".to_string());
            
            // Get the first_run_completed flag from app_state
            let first_run_completed: i64 = conn
                .query_row("SELECT value FROM app_state WHERE key = 'first_run_completed'", [], |row| row.get(0))
                .unwrap_or(0);
            
            // Insert the legacy source with its config
            conn.execute(
                "INSERT INTO sources (source_type, source_url, api_key, host, searchengine, first_run_completed) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![source_type, url, api_key, host, searchengine, first_run_completed],
            )?;
            
            // Associate existing URLs with this source
            let source_id: i64 = conn.last_insert_rowid();
            conn.execute(
                "UPDATE submitted_urls SET source_id = ?1 WHERE source_id IS NULL",
                [source_id],
            )?;
        }
    }
    
    Ok(())
}

// ============================================================================
// Source management
// ============================================================================

#[derive(Debug, Clone)]
pub struct Source {
    pub id: i64,
    pub source_type: String,
    pub source_url: String,
    pub api_key: String,
    pub host: String,
    pub searchengine: String,
    pub first_run_completed: bool,
}

pub fn get_all_sources(conn: &Connection) -> SqlResult<Vec<Source>> {
    let mut stmt = conn.prepare(
        "SELECT id, source_type, source_url, api_key, host, searchengine, first_run_completed FROM sources ORDER BY id"
    )?;
    let sources = stmt
        .query_map([], |row| {
            Ok(Source {
                id: row.get(0)?,
                source_type: row.get(1)?,
                source_url: row.get(2)?,
                api_key: row.get(3)?,
                host: row.get(4)?,
                searchengine: row.get(5)?,
                first_run_completed: row.get::<_, i64>(6)? == 1,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(sources)
}

pub fn add_source(conn: &Connection, source_type: &str, source_url: &str, api_key: &str, host: &str, searchengine: &str) -> SqlResult<i64> {
    conn.execute(
        "INSERT INTO sources (source_type, source_url, api_key, host, searchengine) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![source_type, source_url, api_key, host, searchengine],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_source(conn: &Connection, id: i64, source_type: &str, source_url: &str, api_key: &str, host: &str, searchengine: &str) -> SqlResult<bool> {
    let rows = conn.execute(
        "UPDATE sources SET source_type = ?1, source_url = ?2, api_key = ?3, host = ?4, searchengine = ?5 WHERE id = ?6",
        rusqlite::params![source_type, source_url, api_key, host, searchengine, id],
    )?;
    Ok(rows > 0)
}

pub fn remove_source(conn: &Connection, id: i64) -> SqlResult<bool> {
    // First delete all URLs associated with this source
    conn.execute("DELETE FROM submitted_urls WHERE source_id = ?1", [id])?;
    // Then delete the source
    let rows = conn.execute("DELETE FROM sources WHERE id = ?1", [id])?;
    Ok(rows > 0)
}

pub fn source_exists(conn: &Connection, source_url: &str) -> SqlResult<bool> {
    let result: Option<bool> = conn.query_row(
        "SELECT 1 FROM sources WHERE source_url = ?1",
        [source_url],
        |_| Ok(true),
    ).optional()?;
    Ok(result.is_some())
}

// ============================================================================
// First run detection (per source)
// ============================================================================

pub fn is_source_first_run(conn: &Connection, source_id: i64) -> SqlResult<bool> {
    let result: Result<i64, _> = conn.query_row(
        "SELECT first_run_completed FROM sources WHERE id = ?1",
        [source_id],
        |row| row.get(0),
    );
    match result {
        Ok(1) => Ok(false), // first run already completed
        _ => Ok(true),      // first run not completed or no record
    }
}

pub fn mark_source_first_run_completed(conn: &Connection, source_id: i64) -> SqlResult<()> {
    conn.execute(
        "UPDATE sources SET first_run_completed = 1 WHERE id = ?1",
        [source_id],
    )?;
    Ok(())
}

// ============================================================================
// URL management
// ============================================================================

/// Get URLs with dates for a specific source
pub fn get_urls_with_dates_for_source(conn: &Connection, source_id: i64) -> SqlResult<HashMap<String, Option<String>>> {
    let mut stmt = conn.prepare("SELECT url, last_modified FROM submitted_urls WHERE source_id = ?1")?;
    let map = stmt
        .query_map([source_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(map)
}

/// Add URL with its modification date for a specific source
pub fn add_url_with_date_for_source(conn: &Connection, source_id: i64, url: &str, last_modified: Option<&str>) -> SqlResult<()> {
    conn.execute(
        "INSERT INTO submitted_urls (source_id, url, last_modified) VALUES (?1, ?2, ?3)
         ON CONFLICT(source_id, url) DO UPDATE SET last_modified = ?3, submitted_at = strftime('%s', 'now')",
        rusqlite::params![source_id, url, last_modified],
    )?;
    Ok(())
}

// ============================================================================
// Database maintenance
// ============================================================================

pub fn clear_database() -> Result<(), Box<dyn std::error::Error>> {
    let path = db_path()?;

    println!(
        "{} {}",
        "⚠ WARNING:".red().bold(),
        "This will delete all stored URLs and sources from the database!".red()
    );
    println!(
        "{}",
        "This is a destructive operation and cannot be undone.".yellow()
    );
    println!("Database path: {}\n", path.display().to_string().dimmed());

    if !path.exists() {
        println!(
            "{} Database does not exist. Nothing to clear.",
            "ℹ".cyan().bold()
        );
        return Ok(());
    }

    if Confirm::new()
        .with_prompt("Are you sure you want to clear the database?")
        .default(false)
        .interact()?
    {
        let conn = Connection::open(&path)?;
        conn.execute("DELETE FROM submitted_urls", [])?;
        conn.execute("DELETE FROM sources", [])?;
        conn.execute("DELETE FROM app_state", [])?;

        println!(
            "{} Database cleared. URLs, sources, and app state removed.",
            "✓".green().bold()
        );
        println!(
            "{} The next run will be treated as a first run.",
            "ℹ".cyan().bold()
        );
    } else {
        println!("{} Operation cancelled.", "ℹ".cyan().bold());
    }

    Ok(())
}
