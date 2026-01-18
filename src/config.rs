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
use rusqlite::Connection;
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub api_key: String,
    pub source_type: SourceType,
    pub source_url: String,
    pub host: String,
    pub searchengine: String,
}

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

fn get_config_value(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM config WHERE key = ?1",
        [key],
        |row| row.get(0),
    )
    .ok()
}

fn set_config_value(conn: &Connection, key: &str, value: &str) -> Result<(), Box<dyn std::error::Error>> {
    conn.execute(
        "INSERT INTO config (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    )?;
    Ok(())
}

pub fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    let conn = db::init_db()?;
    
    let source_type = match get_config_value(&conn, "source_type").as_deref() {
        Some("sitemap") => SourceType::Sitemap,
        _ => SourceType::Feed,
    };
    
    // Support legacy "rss_feed" key for backwards compatibility
    let source_url = get_config_value(&conn, "source_url")
        .or_else(|| get_config_value(&conn, "rss_feed"))
        .unwrap_or_default();
    
    Ok(Config {
        api_key: get_config_value(&conn, "api_key").unwrap_or_default(),
        source_type,
        source_url,
        host: get_config_value(&conn, "host").unwrap_or_default(),
        searchengine: get_config_value(&conn, "searchengine").unwrap_or_default(),
    })
}

pub fn save_config(cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let conn = db::init_db()?;
    
    set_config_value(&conn, "api_key", &cfg.api_key)?;
    set_config_value(&conn, "source_type", match cfg.source_type {
        SourceType::Feed => "feed",
        SourceType::Sitemap => "sitemap",
    })?;
    set_config_value(&conn, "source_url", &cfg.source_url)?;
    set_config_value(&conn, "host", &cfg.host)?;
    set_config_value(&conn, "searchengine", &cfg.searchengine)?;
    
    println!(
        "{} Configuration saved to database",
        "✓".green().bold()
    );
    Ok(())
}

pub fn validate_config(cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let mut missing = Vec::new();

    if cfg.api_key.is_empty() {
        missing.push("api_key");
    }
    if cfg.source_url.is_empty() {
        missing.push("source_url (feed or sitemap URL)");
    }
    if cfg.host.is_empty() {
        missing.push("host");
    }
    if cfg.searchengine.is_empty() {
        missing.push("searchengine");
    }

    if !missing.is_empty() {
        return Err(format!(
            "Missing required configuration: {}. Run '{} config' to configure.",
            missing.join(", "),
            env!("CARGO_PKG_NAME")
        )
        .into());
    }

    Ok(())
}

pub fn is_config_complete() -> bool {
    match load_config() {
        Ok(cfg) => {
            !cfg.api_key.is_empty()
                && !cfg.source_url.is_empty()
                && !cfg.host.is_empty()
                && !cfg.searchengine.is_empty()
        }
        Err(_) => false,
    }
}

pub fn edit_config() -> Result<(), Box<dyn std::error::Error>> {
    let mut cfg = load_config()?;

    println!(
        "{} IndexNow Configuration Editor",
        "═".repeat(40).blue().bold()
    );
    println!("Press Enter to keep current value (shown in brackets)\n");

    // API Key (required)
    let current_key = if cfg.api_key.is_empty() {
        "none".to_string()
    } else {
        format!("{}...", &cfg.api_key[..cfg.api_key.len().min(8)])
    };
    loop {
        let api_key: String = Input::new()
            .with_prompt(format!("API Key [{}]", current_key))
            .allow_empty(true)
            .interact_text()?;
        if !api_key.is_empty() {
            cfg.api_key = api_key;
            break;
        } else if !cfg.api_key.is_empty() {
            break;
        }
        println!("{} API Key is required.", "⚠".yellow().bold());
    }

    // Source Type Selection
    println!("\n{}", "URL Source Type:".bold());
    let source_options = vec!["RSS/Atom/JSON Feed", "Sitemap XML"];
    let current_idx = match cfg.source_type {
        SourceType::Feed => 0,
        SourceType::Sitemap => 1,
    };
    let selection = Select::new()
        .with_prompt("Select source type")
        .items(&source_options)
        .default(current_idx)
        .interact()?;
    
    cfg.source_type = match selection {
        0 => SourceType::Feed,
        _ => SourceType::Sitemap,
    };

    // Source URL (required, validated)
    let source_label = match cfg.source_type {
        SourceType::Feed => "RSS/Atom/JSON Feed URL",
        SourceType::Sitemap => "Sitemap URL (e.g., https://example.com/sitemap.xml)",
    };
    let current_source = if cfg.source_url.is_empty() {
        "none".to_string()
    } else {
        cfg.source_url.clone()
    };
    loop {
        let source_url: String = Input::new()
            .with_prompt(format!("{} [{}]", source_label, current_source))
            .allow_empty(true)
            .interact_text()?;
        
        let url_to_validate = if !source_url.is_empty() {
            source_url.clone()
        } else if !cfg.source_url.is_empty() {
            cfg.source_url.clone()
        } else {
            println!("{} Source URL is required.", "⚠".yellow().bold());
            continue;
        };
        
        // Validate the URL
        match validate_source_url(&url_to_validate, cfg.source_type) {
            Ok(validated_url) => {
                cfg.source_url = validated_url;
                println!("  {} URL is valid and accessible.", "✓".green().bold());
                break;
            }
            Err(e) => {
                println!("{} {}", "✗".red().bold(), e);
                // Clear the current source if user entered a new invalid one
                if !source_url.is_empty() {
                    continue;
                }
                // If they pressed enter with existing value that's now invalid, ask again
                continue;
            }
        }
    }

    // Auto-extract host from source URL
    let extracted_host = extract_host_from_url(&cfg.source_url);
    if let Some(ref host) = extracted_host {
        cfg.host = host.clone();
        println!(
            "{} Host auto-detected from URL: {}",
            "✓".green().bold(),
            host.cyan()
        );
    } else {
        // Host not detected, ask user
        let current_host = if cfg.host.is_empty() {
            "none".to_string()
        } else {
            cfg.host.clone()
        };
        loop {
            let host: String = Input::new()
                .with_prompt(format!("Host (your domain, e.g., example.com) [{}]", current_host))
                .allow_empty(true)
                .interact_text()?;
            if !host.is_empty() {
                cfg.host = host;
                break;
            } else if !cfg.host.is_empty() {
                break;
            }
            println!("{} Host is required.", "⚠".yellow().bold());
        }
    }

    // Search Engine
    let current_engine = if cfg.searchengine.is_empty() {
        "api.indexnow.org".to_string()
    } else {
        cfg.searchengine.clone()
    };
    println!("\n{}", "Available IndexNow endpoints:".dimmed());
    println!("  • api.indexnow.org (recommended, forwards to all)");
    println!("  • www.bing.com");
    println!("  • yandex.com");
    println!("  • search.seznam.cz\n");

    let searchengine: String = Input::new()
        .with_prompt(format!("Search Engine Host [{}]", current_engine))
        .allow_empty(true)
        .interact_text()?;
    if !searchengine.is_empty() {
        cfg.searchengine = searchengine;
    } else if cfg.searchengine.is_empty() {
        cfg.searchengine = "api.indexnow.org".to_string();
    }

    // Confirm and save
    println!("\n{}", "Configuration Summary:".bold());
    println!("  API Key:       {}", mask_key(&cfg.api_key));
    println!("  Source Type:   {}", cfg.source_type.to_string().cyan());
    println!("  Source URL:    {}", cfg.source_url);
    println!("  Host:          {}", cfg.host);
    println!("  Search Engine: {}", cfg.searchengine);

    if Confirm::new()
        .with_prompt("Save this configuration?")
        .default(true)
        .interact()?
    {
        save_config(&cfg)?;
    } else {
        println!("{} Configuration not saved.", "⚠".yellow().bold());
    }

    Ok(())
}

pub fn list_config() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = load_config()?;

    println!(
        "{} IndexNow Configuration",
        "═".repeat(40).blue().bold()
    );
    println!("Stored in: {}\n", "SQLite database".dimmed());

    if cfg.api_key.is_empty()
        && cfg.source_url.is_empty()
        && cfg.host.is_empty()
        && cfg.searchengine.is_empty()
    {
        println!(
            "{} No configuration found. Run '{} config' to configure.",
            "⚠".yellow().bold(),
            env!("CARGO_PKG_NAME")
        );
        return Ok(());
    }

    println!(
        "  {} {}",
        "API Key:".bold(),
        if cfg.api_key.is_empty() {
            "(not set)".red().to_string()
        } else {
            mask_key(&cfg.api_key)
        }
    );
    println!(
        "  {} {}",
        "Source Type:".bold(),
        cfg.source_type.to_string().cyan()
    );
    println!(
        "  {} {}",
        "Source URL:".bold(),
        if cfg.source_url.is_empty() {
            "(not set)".red().to_string()
        } else {
            cfg.source_url.green().to_string()
        }
    );
    println!(
        "  {} {}",
        "Host:".bold(),
        if cfg.host.is_empty() {
            "(not set)".red().to_string()
        } else {
            cfg.host.green().to_string()
        }
    );
    println!(
        "  {} {}",
        "Search Engine:".bold(),
        if cfg.searchengine.is_empty() {
            "(not set)".red().to_string()
        } else {
            cfg.searchengine.green().to_string()
        }
    );

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
