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
use rusqlite::{Connection, Result as SqlResult};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

fn db_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let data_dir = dirs::data_dir()
        .ok_or("Could not determine data directory")?
        .join("ixfeed");
    fs::create_dir_all(&data_dir)?;
    Ok(data_dir.join("ixfeed.db"))
}

pub fn init_db() -> Result<Connection, Box<dyn std::error::Error>> {
    let path = db_path()?;
    let conn = Connection::open(&path)?;

    // URLs table with last_modified tracking
    conn.execute(
        "CREATE TABLE IF NOT EXISTS submitted_urls (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            url TEXT UNIQUE NOT NULL,
            last_modified TEXT,
            submitted_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        )",
        [],
    )?;

    // Config table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS config (
            key TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL
        )",
        [],
    )?;

    // App state table (for first_run flag, etc.)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS app_state (
            key TEXT PRIMARY KEY NOT NULL,
            value INTEGER NOT NULL
        )",
        [],
    )?;

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

    Ok(conn)
}

// ============================================================================
// First run detection
// ============================================================================

pub fn is_first_run(conn: &Connection) -> SqlResult<bool> {
    let result: Result<i64, _> = conn.query_row(
        "SELECT value FROM app_state WHERE key = 'first_run_completed'",
        [],
        |row| row.get(0),
    );
    match result {
        Ok(1) => Ok(false), // first run already completed
        _ => Ok(true),      // first run not completed or no record
    }
}

pub fn mark_first_run_completed(conn: &Connection) -> SqlResult<()> {
    conn.execute(
        "INSERT INTO app_state (key, value) VALUES ('first_run_completed', 1)
         ON CONFLICT(key) DO UPDATE SET value = 1",
        [],
    )?;
    Ok(())
}

// ============================================================================
// URL management
// ============================================================================

#[allow(dead_code)]
pub fn get_all_urls(conn: &Connection) -> SqlResult<Vec<String>> {
    let mut stmt = conn.prepare("SELECT url FROM submitted_urls")?;
    let urls = stmt
        .query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(urls)
}

/// Get all URLs with their last_modified dates as a HashMap for quick lookup
pub fn get_urls_with_dates(conn: &Connection) -> SqlResult<HashMap<String, Option<String>>> {
    let mut stmt = conn.prepare("SELECT url, last_modified FROM submitted_urls")?;
    let map = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(map)
}

#[allow(dead_code)]
pub fn add_url(conn: &Connection, url: &str) -> SqlResult<()> {
    conn.execute(
        "INSERT OR IGNORE INTO submitted_urls (url) VALUES (?1)",
        [url],
    )?;
    Ok(())
}

/// Add URL with its modification date
pub fn add_url_with_date(conn: &Connection, url: &str, last_modified: Option<&str>) -> SqlResult<()> {
    conn.execute(
        "INSERT INTO submitted_urls (url, last_modified) VALUES (?1, ?2)
         ON CONFLICT(url) DO UPDATE SET last_modified = ?2, submitted_at = strftime('%s', 'now')",
        rusqlite::params![url, last_modified],
    )?;
    Ok(())
}

/// Update the last_modified date for an existing URL
#[allow(dead_code)]
pub fn update_url_date(conn: &Connection, url: &str, last_modified: Option<&str>) -> SqlResult<()> {
    conn.execute(
        "UPDATE submitted_urls SET last_modified = ?2, submitted_at = strftime('%s', 'now') WHERE url = ?1",
        rusqlite::params![url, last_modified],
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
        "This will delete all stored URLs from the database!".red()
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
        conn.execute("DELETE FROM app_state", [])?;

        println!(
            "{} Database cleared. URLs and app state removed.",
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
