//! Configuration management for ixfeed

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

use crate::db;
use colored::*;
use dialoguer::{Confirm, Input, Select};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SourceType {
    #[default]
    Feed,
    Sitemap,
}

impl std::fmt::Display for SourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceType::Feed => write!(f, "RSS/Atom/JSON Feed"),
            SourceType::Sitemap => write!(f, "Sitemap XML"),
        }
    }
}

// Re-export Source from db module for convenience
pub use crate::db::Source;

/// Extract host (domain) from a URL
fn extract_host_from_url(url: &str) -> Option<String> {
    Url::parse(url).ok().and_then(|u| u.host_str().map(|h| h.to_string()))
}

/// Validate source URL: must be valid format, HTTPS (auto-upgrade from HTTP), and accessible
/// Returns the validated (possibly upgraded) URL on success
fn validate_source_url(url: &str, source_type: SourceType) -> Result<String, String> {
    // Auto-add https:// if no scheme is present
    let url_with_scheme = if !url.contains("://") {
        let fixed = format!("https://{}", url);
        println!(
            "  {} Added HTTPS prefix: {}",
            "↑".cyan(),
            fixed
        );
        fixed
    } else {
        url.to_string()
    };
    
    // Parse URL
    let mut parsed = Url::parse(&url_with_scheme).map_err(|e| format!("Invalid URL format: {}", e))?;
    
    // Auto-upgrade HTTP to HTTPS
    if parsed.scheme() == "http" {
        parsed.set_scheme("https").map_err(|_| "Failed to upgrade to HTTPS")?;
        println!(
            "  {} Auto-upgraded to HTTPS: {}",
            "↑".cyan(),
            parsed.as_str()
        );
    } else if parsed.scheme() != "https" {
        return Err(format!("URL must use HTTP or HTTPS, got: {}", parsed.scheme()));
    }
    
    // Must have a host
    if parsed.host_str().is_none() {
        return Err("URL must have a valid host".to_string());
    }
    
    let final_url = parsed.to_string();
    
    // Check if accessible
    println!("  {} Validating URL...", "→".blue());
    
    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(format!(
            "{}/{}",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let response = client
        .get(&final_url)
        .send()
        .map_err(|e| {
            format!(
                "Could not access URL: {}\n    Please verify the URL is correct and try again.",
                e
            )
        })?;
    
    if !response.status().is_success() {
        return Err(format!(
            "URL returned HTTP {} - {}\n    Please verify the URL exists and is publicly accessible.",
            response.status().as_u16(),
            response.status().canonical_reason().unwrap_or("Unknown")
        ));
    }
    
    // Optionally validate content type
    if let Some(content_type) = response.headers().get("content-type") {
        let ct = content_type.to_str().unwrap_or("");
        match source_type {
            SourceType::Feed => {
                // RSS/Atom/JSON feeds can have various content types
                // application/rss+xml, application/atom+xml, application/feed+json,
                // application/xml, text/xml, application/json, etc.
            }
            SourceType::Sitemap => {
                if !ct.contains("xml") && !ct.contains("text/plain") {
                    println!(
                        "  {} Content-Type is '{}', expected XML. Proceeding anyway.",
                        "⚠".yellow(),
                        ct
                    );
                }
            }
        }
    }
    
    Ok(final_url)
}

/// Check if there are any sources configured
pub fn has_sources() -> bool {
    match db::init_db() {
        Ok(conn) => {
            let sources = db::get_all_sources(&conn).unwrap_or_default();
            !sources.is_empty()
        }
        Err(_) => false,
    }
}

/// Get all configured sources
pub fn get_sources() -> Result<Vec<Source>, Box<dyn std::error::Error>> {
    let conn = db::init_db()?;
    Ok(db::get_all_sources(&conn)?)
}

/// Add a new source (feed or sitemap) with per-source config
pub fn add_source(source_type: SourceType, source_url: &str, api_key: &str, host: &str, searchengine: &str) -> Result<i64, Box<dyn std::error::Error>> {
    let conn = db::init_db()?;
    
    // Check if source already exists
    if db::source_exists(&conn, source_url)? {
        return Err(format!("Source already exists: {}", source_url).into());
    }
    
    let type_str = match source_type {
        SourceType::Feed => "feed",
        SourceType::Sitemap => "sitemap",
    };
    
    let id = db::add_source(&conn, type_str, source_url, api_key, host, searchengine)?;
    Ok(id)
}

/// Remove a source by ID
pub fn remove_source(id: i64) -> Result<bool, Box<dyn std::error::Error>> {
    let conn = db::init_db()?;
    Ok(db::remove_source(&conn, id)?)
}

pub fn edit_config() -> Result<(), Box<dyn std::error::Error>> {
    let sources = get_sources()?;

    if sources.is_empty() {
        println!(
            "{} No sources configured. Run '{} --add' to add a source first.",
            "⚠".yellow().bold(),
            env!("CARGO_PKG_NAME")
        );
        return Ok(());
    }

    println!(
        "{} Edit Source Configuration",
        "═".repeat(40).blue().bold()
    );

    // List available sources
    println!("\n{}", "Available sources:".bold());
    let source_labels: Vec<String> = sources
        .iter()
        .map(|s| {
            let type_str = match s.source_type.as_str() {
                "sitemap" => "Sitemap",
                _ => "Feed",
            };
            format!("[ID {}] {} - {}", s.id, type_str, s.source_url)
        })
        .collect();

    let selection = Select::new()
        .with_prompt("Select source to edit")
        .items(&source_labels)
        .interact()?;

    let source = &sources[selection];

    println!("\n{}", "Edit settings (press Enter to keep current value):".dimmed());

    // Source Type
    let type_options = vec!["RSS/Atom/JSON Feed", "Sitemap XML"];
    let current_type_idx = if source.source_type == "sitemap" { 1 } else { 0 };
    let type_selection = Select::new()
        .with_prompt(format!("Source Type [{}]", type_options[current_type_idx]))
        .items(&type_options)
        .default(current_type_idx)
        .interact()?;
    let new_source_type = if type_selection == 1 { "sitemap" } else { "feed" };

    // Source URL
    let new_url: String = Input::new()
        .with_prompt(format!("Source URL [{}]", source.source_url))
        .allow_empty(true)
        .interact_text()?;
    let new_url = if new_url.is_empty() {
        source.source_url.clone()
    } else {
        // Validate the new URL if changed
        let stype = if new_source_type == "sitemap" { SourceType::Sitemap } else { SourceType::Feed };
        match validate_source_url(&new_url, stype) {
            Ok(validated) => validated,
            Err(e) => {
                println!("{} {}", "✗".red().bold(), e);
                println!("Keeping original URL.");
                source.source_url.clone()
            }
        }
    };

    // API Key
    let current_key = if source.api_key.is_empty() {
        "none".to_string()
    } else {
        mask_key(&source.api_key)
    };
    let new_api_key: String = Input::new()
        .with_prompt(format!("API Key [{}]", current_key))
        .allow_empty(true)
        .interact_text()?;
    let new_api_key = if new_api_key.is_empty() {
        source.api_key.clone()
    } else {
        new_api_key
    };

    // Host
    let current_host = if source.host.is_empty() {
        "none".to_string()
    } else {
        source.host.clone()
    };
    let new_host: String = Input::new()
        .with_prompt(format!("Host [{}]", current_host))
        .allow_empty(true)
        .interact_text()?;
    let new_host = if new_host.is_empty() {
        source.host.clone()
    } else {
        new_host
    };

    // Search Engine
    let current_engine = if source.searchengine.is_empty() {
        "api.indexnow.org".to_string()
    } else {
        source.searchengine.clone()
    };
    println!("\n{}", "Available IndexNow endpoints:".dimmed());
    println!("  • api.indexnow.org (recommended, forwards to all)");
    println!("  • www.bing.com");
    println!("  • yandex.com");
    println!("  • search.seznam.cz\n");

    let new_searchengine: String = Input::new()
        .with_prompt(format!("Search Engine Host [{}]", current_engine))
        .allow_empty(true)
        .interact_text()?;
    let new_searchengine = if new_searchengine.is_empty() {
        if source.searchengine.is_empty() {
            "api.indexnow.org".to_string()
        } else {
            source.searchengine.clone()
        }
    } else {
        new_searchengine
    };

    // Summary and confirm
    println!("\n{}", "Updated Configuration:".bold());
    println!("  Type:          {}", if new_source_type == "sitemap" { "Sitemap".cyan() } else { "Feed".cyan() });
    println!("  URL:           {}", new_url.green());
    println!("  API Key:       {}", mask_key(&new_api_key));
    println!("  Host:          {}", new_host.green());
    println!("  Search Engine: {}", new_searchengine.green());

    if Confirm::new()
        .with_prompt("Save changes?")
        .default(true)
        .interact()?
    {
        let conn = db::init_db()?;
        db::update_source(&conn, source.id, new_source_type, &new_url, &new_api_key, &new_host, &new_searchengine)?;
        println!(
            "{} Configuration saved.",
            "✓".green().bold()
        );
    } else {
        println!("{} Configuration not saved.", "⚠".yellow().bold());
    }

    Ok(())
}

/// Interactive source addition
pub fn add_source_interactive() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "{} Add New Source (Feed or Sitemap)",
        "═".repeat(35).blue().bold()
    );

    // Source Type Selection
    println!("\n{}", "URL Source Type:".bold());
    let source_options = vec!["RSS/Atom/JSON Feed", "Sitemap XML"];
    let selection = Select::new()
        .with_prompt("Select source type")
        .items(&source_options)
        .default(0)
        .interact()?;
    
    let source_type = match selection {
        0 => SourceType::Feed,
        _ => SourceType::Sitemap,
    };

    // Source URL (required, validated)
    let source_label = match source_type {
        SourceType::Feed => "RSS/Atom/JSON Feed URL",
        SourceType::Sitemap => "Sitemap URL (e.g., https://example.com/sitemap.xml)",
    };
    
    let validated_url = loop {
        let source_url: String = Input::new()
            .with_prompt(source_label)
            .interact_text()?;
        
        if source_url.is_empty() {
            println!("{} Source URL is required.", "⚠".yellow().bold());
            continue;
        }
        
        // Check if already exists
        let conn = db::init_db()?;
        if db::source_exists(&conn, &source_url)? {
            println!("{} This source already exists.", "⚠".yellow().bold());
            continue;
        }
        
        // Validate the URL
        match validate_source_url(&source_url, source_type) {
            Ok(validated_url) => {
                println!("  {} URL is valid and accessible.", "✓".green().bold());
                break validated_url;
            }
            Err(e) => {
                println!("{} {}", "✗".red().bold(), e);
                continue;
            }
        }
    };

    // Extract host suggestion from URL
    let suggested_host = extract_host_from_url(&validated_url).unwrap_or_default();

    println!("\n{}", "IndexNow API Settings for this source:".bold());
    
    // API Key (required)
    let api_key = loop {
        let key: String = Input::new()
            .with_prompt("API Key (your IndexNow key)")
            .interact_text()?;
        if !key.is_empty() {
            break key;
        }
        println!("{} API Key is required.", "⚠".yellow().bold());
    };

    // Host (required)
    let host = loop {
        let h: String = Input::new()
            .with_prompt(format!("Host (your domain) [{}]", if suggested_host.is_empty() { "required".to_string() } else { suggested_host.clone() }))
            .allow_empty(true)
            .interact_text()?;
        if !h.is_empty() {
            break h;
        } else if !suggested_host.is_empty() {
            break suggested_host.clone();
        }
        println!("{} Host is required.", "⚠".yellow().bold());
    };

    // Search Engine
    println!("\n{}", "Available IndexNow endpoints:".dimmed());
    println!("  • api.indexnow.org (recommended, forwards to all)");
    println!("  • www.bing.com");
    println!("  • yandex.com");
    println!("  • search.seznam.cz\n");

    let searchengine: String = Input::new()
        .with_prompt("Search Engine Host [api.indexnow.org]")
        .allow_empty(true)
        .interact_text()?;
    let searchengine = if searchengine.is_empty() {
        "api.indexnow.org".to_string()
    } else {
        searchengine
    };

    // Summary and confirm
    println!("\n{}", "Source Summary:".bold());
    println!("  Type:          {}", source_type.to_string().cyan());
    println!("  URL:           {}", validated_url.green());
    println!("  API Key:       {}", mask_key(&api_key));
    println!("  Host:          {}", host.green());
    println!("  Search Engine: {}", searchengine.green());

    if Confirm::new()
        .with_prompt("Add this source?")
        .default(true)
        .interact()?
    {
        let id = add_source(source_type, &validated_url, &api_key, &host, &searchengine)?;
        
        println!(
            "\n{} Source added successfully (ID: {})",
            "✓".green().bold(),
            id
        );
    } else {
        println!("{} Operation cancelled.", "ℹ".cyan().bold());
    }

    Ok(())
}

/// List all configured sources
pub fn list_sources() -> Result<(), Box<dyn std::error::Error>> {
    let sources = get_sources()?;
    
    println!(
        "{} Configured Sources",
        "═".repeat(40).blue().bold()
    );
    
    if sources.is_empty() {
        println!(
            "\n{} No sources configured. Run '{} --add' to add a source.",
            "⚠".yellow().bold(),
            env!("CARGO_PKG_NAME")
        );
        return Ok(());
    }
    
    println!();
    for source in &sources {
        let type_str = match source.source_type.as_str() {
            "sitemap" => "Sitemap".cyan(),
            _ => "Feed".cyan(),
        };
        let status = if source.first_run_completed {
            "synced".green()
        } else {
            "new".yellow()
        };
        println!(
            "  ID {} [{}] {} ({})",
            source.id.to_string().bold(),
            type_str,
            source.source_url,
            status
        );
        println!("     API Key: {}  Host: {}  Engine: {}",
            mask_key(&source.api_key),
            if source.host.is_empty() { "(not set)".red().to_string() } else { source.host.green().to_string() },
            source.searchengine.dimmed()
        );
    }
    
    println!(
        "\n{} Use '{} -e <ids>' to process specific sources.",
        "ℹ".cyan().bold(),
        env!("CARGO_PKG_NAME")
    );

    Ok(())
}

/// Remove a source interactively
pub fn remove_source_interactive() -> Result<(), Box<dyn std::error::Error>> {
    let sources = get_sources()?;
    
    if sources.is_empty() {
        println!(
            "{} No sources configured.",
            "ℹ".cyan().bold()
        );
        return Ok(());
    }
    
    println!(
        "{} Remove Source",
        "═".repeat(40).blue().bold()
    );
    
    // List sources
    println!("\n{}", "Available sources:".bold());
    for source in &sources {
        let type_str = match source.source_type.as_str() {
            "sitemap" => "Sitemap",
            _ => "Feed",
        };
        println!("  ID {} [{}] {}", source.id, type_str, source.source_url);
    }
    
    // Ask for ID
    let id: String = Input::new()
        .with_prompt("\nEnter source ID to remove (or press Enter to cancel)")
        .allow_empty(true)
        .interact_text()?;
    
    if id.is_empty() {
        println!("{} Operation cancelled.", "ℹ".cyan().bold());
        return Ok(());
    }
    
    let id: i64 = id.parse().map_err(|_| "Invalid ID")?;
    
    // Find the source
    let source = sources.iter().find(|s| s.id == id);
    if source.is_none() {
        return Err(format!("Source with ID {} not found", id).into());
    }
    let source = source.unwrap();
    
    // Confirm
    println!("\n{} This will remove:", "⚠ WARNING:".yellow().bold());
    println!("  Source: {}", source.source_url);
    println!("  And all associated submitted URLs from the database.\n");
    
    if Confirm::new()
        .with_prompt("Are you sure?")
        .default(false)
        .interact()?
    {
        remove_source(id)?;
        println!(
            "{} Source removed.",
            "✓".green().bold()
        );
    } else {
        println!("{} Operation cancelled.", "ℹ".cyan().bold());
    }

    Ok(())
}

pub fn list_config() -> Result<(), Box<dyn std::error::Error>> {
    let sources = get_sources()?;

    println!(
        "{} IndexNow Configuration",
        "═".repeat(40).blue().bold()
    );
    let db_path = db::db_path().map(|p| p.display().to_string()).unwrap_or_else(|_| "unknown".to_string());
    println!("Stored in: SQLite database at {}\n", db_path.dimmed());

    if sources.is_empty() {
        println!(
            "{} No sources configured. Run '{} --add' to add a source.",
            "⚠".yellow().bold(),
            env!("CARGO_PKG_NAME")
        );
        return Ok(());
    }

    println!("{} ({}):", "Sources".bold(), sources.len());
    for source in &sources {
        let type_str = match source.source_type.as_str() {
            "sitemap" => "Sitemap".cyan(),
            _ => "Feed".cyan(),
        };
        let status = if source.first_run_completed {
            "synced".green()
        } else {
            "new".yellow()
        };
        println!(
            "\n  ID {} [{}] {} ({})",
            source.id.to_string().bold(),
            type_str,
            source.source_url.green(),
            status
        );
        println!(
            "     {} {}",
            "API Key:".bold(),
            mask_key(&source.api_key)
        );
        println!(
            "     {} {}",
            "Host:".bold(),
            if source.host.is_empty() {
                "(not set)".red().to_string()
            } else {
                source.host.green().to_string()
            }
        );
        println!(
            "     {} {}",
            "Search Engine:".bold(),
            if source.searchengine.is_empty() {
                "(not set)".red().to_string()
            } else {
                source.searchengine.green().to_string()
            }
        );
    }

    Ok(())
}

fn mask_key(key: &str) -> String {
    if key.is_empty() {
        "(not set)".red().to_string()
    } else if key.len() <= 8 {
        format!("{}***", &key[..2]).green().to_string()
    } else {
        format!("{}...{}", &key[..4], &key[key.len() - 4..])
            .green()
            .to_string()
    }
}
