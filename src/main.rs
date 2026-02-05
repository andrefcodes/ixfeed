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

mod config;
mod db;
mod feed;
mod sitemap;
mod submit;

use clap::Parser;
use colored::*;
use config::SourceType;
use dialoguer::Confirm;
use feed::UrlEntry;
use std::process;
use submit::{SubmitEntry, SubmitReason};

/// IndexNow RSS/Atom/JSON/Sitemap feed submitter
#[derive(Parser)]
#[command(name = "ixfeed")]
#[command(disable_version_flag = true)]
#[command(disable_help_flag = true)]
#[command(about, long_about = None)]
struct Cli {
    /// Edit configuration (API key, host, search engine)
    #[arg(short, long)]
    config: bool,

    /// Show current configuration and sources
    #[arg(short, long)]
    show: bool,

    /// Add a new source (feed or sitemap)
    #[arg(short, long)]
    add: bool,

    /// Remove a source
    #[arg(short, long)]
    remove: bool,

    /// List all configured sources
    #[arg(short, long)]
    list: bool,

    /// Process only specific source entries (comma-separated IDs, e.g., -e 1,2,3)
    #[arg(short, long, value_delimiter = ',')]
    entry: Option<Vec<i64>>,

    /// Clear the database (WARNING: destructive operation)
    #[arg(long)]
    clear_db: bool,

    /// Dry run - show URLs that would be submitted without actually submitting
    #[arg(short, long)]
    dry_run: bool,

    /// Submit URLs without confirmation (for automation)
    #[arg(short, long)]
    unattended: bool,

    /// Show version information
    #[arg(short = 'V', long)]
    version: bool,

    /// Show help information
    #[arg(short = 'H', long)]
    help: bool,
}

fn main() {
    let cli = Cli::parse();

    if cli.help {
        print_help();
        return;
    }

    if cli.version {
        println!(
            "{} v{} {}",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
            "(ALPHA)".yellow().bold()
        );
        println!();
        println!(
            "{}  {}: This software is in alpha stage.",
            "⚠️".yellow(),
            "WARNING".yellow().bold()
        );
        println!("   Features may be incomplete, unstable, or change without notice.");
        println!();
        println!("Copyright (C) 2026 Andre Franca");
        println!("Licensed under the GNU AGPL v3.0 or later.");
        println!("See <https://www.gnu.org/licenses/agpl-3.0.html> for details.");
        return;
    }

    if cli.config {
        if let Err(e) = config::edit_config() {
            eprintln!("{}: {}", "Error".red().bold(), e);
            process::exit(1);
        }
        return;
    }

    if cli.show {
        if let Err(e) = config::list_config() {
            eprintln!("{}: {}", "Error".red().bold(), e);
            process::exit(1);
        }
        return;
    }

    if cli.add {
        if let Err(e) = config::add_source_interactive() {
            eprintln!("{}: {}", "Error".red().bold(), e);
            process::exit(1);
        }
        return;
    }

    if cli.remove {
        if let Err(e) = config::remove_source_interactive() {
            eprintln!("{}: {}", "Error".red().bold(), e);
            process::exit(1);
        }
        return;
    }

    if cli.list {
        if let Err(e) = config::list_sources() {
            eprintln!("{}: {}", "Error".red().bold(), e);
            process::exit(1);
        }
        return;
    }

    if cli.clear_db {
        if let Err(e) = db::clear_database() {
            eprintln!("{}: {}", "Error".red().bold(), e);
            process::exit(1);
        }
        return;
    }

    if cli.dry_run {
        if let Err(e) = run_dry_run(cli.entry.as_deref()) {
            eprintln!("{}: {}", "Error".red().bold(), e);
            process::exit(1);
        }
        return;
    }

    if cli.unattended {
        if let Err(e) = run_unattended_submission(cli.entry.as_deref()) {
            eprintln!("{}: {}", "Error".red().bold(), e);
            process::exit(1);
        }
        return;
    }

    // Default: run submission workflow
    {
        // Check if we have any sources
        if !config::has_sources() {
            println!(
                "{} No sources configured. Let's add one.\n",
                "ℹ".cyan().bold()
            );
            if let Err(e) = config::add_source_interactive() {
                eprintln!("{}: {}", "Error".red().bold(), e);
                process::exit(1);
            }
            println!();
        }
        
        // Now run the submission workflow
        if let Err(e) = run_submission(cli.entry.as_deref()) {
            eprintln!("{}: {}", "Error".red().bold(), e);
            process::exit(1);
        }
    }
}

fn print_help() {
    println!(
        "{} - RSS/Atom/JSON feed and sitemap watcher for IndexNow",
        env!("CARGO_PKG_NAME").bold()
    );
    println!();
    println!("{}", "Usage:".bold());
    println!("  {} [OPTIONS]", env!("CARGO_PKG_NAME"));
    println!();
    println!("{}", "Options:".bold());
    println!("  {}, {}     Edit configuration (API key, host, search engine)", "-c".cyan(), "--config".cyan());
    println!("  {}, {}       Show current configuration and sources", "-s".cyan(), "--show".cyan());
    println!("  {}, {}        Add a new source (feed or sitemap)", "-a".cyan(), "--add".cyan());
    println!("  {}, {}     Remove a source", "-r".cyan(), "--remove".cyan());
    println!("  {}, {}       List all configured sources", "-l".cyan(), "--list".cyan());
    println!("  {}, {} {} Process only specific sources (comma-separated IDs)", "-e".cyan(), "--entry".cyan(), "<IDs>".dimmed());
    println!("      {}   Clear the database (WARNING: destructive operation)", "--clear-db".cyan());
    println!("  {}, {}    Dry run - show URLs that would be submitted", "-d".cyan(), "--dry-run".cyan());
    println!("  {}, {} Submit URLs without confirmation (for automation)", "-u".cyan(), "--unattended".cyan());
    println!("  {}, {}    Show version information", "-V".cyan(), "--version".cyan());
    println!("  {}, {}       Show this help message", "-H".cyan(), "--help".cyan());
}

fn get_sources_to_process(entry_filter: Option<&[i64]>) -> Result<Vec<db::Source>, Box<dyn std::error::Error>> {
    let all_sources = config::get_sources()?;
    
    if all_sources.is_empty() {
        return Err("No sources configured. Run 'ixfeed --add' to add a source.".into());
    }
    
    match entry_filter {
        Some(ids) => {
            let filtered: Vec<db::Source> = all_sources
                .into_iter()
                .filter(|s| ids.contains(&s.id))
                .collect();
            
            if filtered.is_empty() {
                return Err(format!(
                    "No sources found with IDs: {:?}. Run 'ixfeed --list' to see available sources.",
                    ids
                ).into());
            }
            
            Ok(filtered)
        }
        None => Ok(all_sources),
    }
}

fn run_dry_run(entry_filter: Option<&[i64]>) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize database
    let conn = db::init_db()?;
    
    // Get sources to process
    let sources = get_sources_to_process(entry_filter)?;
    
    // Validate that all sources have required config
    for source in &sources {
        if source.api_key.is_empty() || source.host.is_empty() {
            return Err(format!(
                "Source {} ({}) is missing required configuration (api_key or host). Run '{} --config' to configure.",
                source.id, source.source_url, env!("CARGO_PKG_NAME")
            ).into());
        }
    }

    println!(
        "{} {} Dry Run Mode {}",
        "═".repeat(15).blue(),
        "DRY RUN".yellow().bold(),
        "═".repeat(15).blue()
    );
    println!("{}", "No URLs will be submitted.\n".dimmed());
    
    if sources.len() > 1 {
        println!(
            "{} Processing {} sources...\n",
            "ℹ".cyan().bold(),
            sources.len()
        );
    }
    
    for source in &sources {
        dry_run_source(&conn, source)?;
        if sources.len() > 1 {
            println!();
        }
    }

    println!(
        "\n{} To actually submit, run: {}",
        "→".blue().bold(),
        env!("CARGO_PKG_NAME").cyan()
    );

    Ok(())
}

fn dry_run_source(conn: &rusqlite::Connection, source: &db::Source) -> Result<(), Box<dyn std::error::Error>> {
    let source_type = if source.source_type == "sitemap" {
        SourceType::Sitemap
    } else {
        SourceType::Feed
    };
    let source_type_str = if source.source_type == "sitemap" { "sitemap" } else { "feed" };
    
    println!(
        "{} [{}] Fetching {} from {}...",
        "→".blue().bold(),
        source.id.to_string().bold(),
        source_type_str,
        source.source_url
    );

    let entries: Vec<UrlEntry> = match source_type {
        SourceType::Feed => feed::fetch_feed_urls(&source.source_url)?,
        SourceType::Sitemap => sitemap::fetch_sitemap_urls(&source.source_url)?,
    };

    if entries.is_empty() {
        println!(
            "  {} No URLs found in {}.",
            "⚠".yellow().bold(),
            source_type_str
        );
        return Ok(());
    }

    // Check if first run for this source
    let is_first_run = db::is_source_first_run(conn, source.id)?;

    if is_first_run {
        println!(
            "  {} First run detected. {} URL(s) found:\n",
            "ℹ".cyan().bold(),
            entries.len()
        );
        
        for (i, entry) in entries.iter().take(10).enumerate() {
            let date_str = entry.date.as_deref().unwrap_or("no date");
            println!(
                "    {}. {} {}",
                (i + 1).to_string().dimmed(),
                entry.url.green(),
                format!("({})", date_str).dimmed()
            );
        }
        if entries.len() > 10 {
            println!("    {} ... and {} more", "".dimmed(), entries.len() - 10);
        }

        println!(
            "\n  {} On actual run, you would be asked to confirm submission of all {} URL(s).",
            "ℹ".cyan().bold(),
            entries.len()
        );
    } else {
        // Check for new or modified URLs
        let stored_urls = db::get_urls_with_dates_for_source(conn, source.id)?;
        
        let mut new_urls: Vec<&UrlEntry> = Vec::new();
        let mut modified_urls: Vec<(&UrlEntry, Option<String>)> = Vec::new();

        for entry in &entries {
            if let Some(stored_date) = stored_urls.get(&entry.url) {
                // URL exists - check if modified
                if entry.date.is_some() && entry.date != *stored_date {
                    modified_urls.push((entry, stored_date.clone()));
                }
            } else {
                // New URL
                new_urls.push(entry);
            }
        }

        let total_to_submit = new_urls.len() + modified_urls.len();

        if total_to_submit == 0 {
            println!(
                "  {} No new or modified URLs to submit. All {} URL(s) are up to date.",
                "✓".green().bold(),
                entries.len()
            );
            return Ok(());
        }

        println!(
            "  {} Would submit {} URL(s):\n",
            "ℹ".cyan().bold(),
            total_to_submit
        );

        if !new_urls.is_empty() {
            println!("  {} ({}):", "New URLs".green().bold(), new_urls.len());
            for (i, entry) in new_urls.iter().take(5).enumerate() {
                let date_str = entry.date.as_deref().unwrap_or("no date");
                println!(
                    "    {}. {} {}",
                    (i + 1).to_string().dimmed(),
                    entry.url,
                    format!("({})", date_str).dimmed()
                );
            }
            if new_urls.len() > 5 {
                println!("    {} ... and {} more", "".dimmed(), new_urls.len() - 5);
            }
        }

        if !modified_urls.is_empty() {
            if !new_urls.is_empty() {
                println!();
            }
            println!("  {} ({}):", "Modified URLs".yellow().bold(), modified_urls.len());
            for (i, (entry, old_date)) in modified_urls.iter().take(5).enumerate() {
                let old_str = old_date.as_deref().unwrap_or("unknown");
                let new_str = entry.date.as_deref().unwrap_or("unknown");
                println!(
                    "    {}. {} {} → {}",
                    (i + 1).to_string().dimmed(),
                    entry.url,
                    old_str.dimmed(),
                    new_str.cyan()
                );
            }
            if modified_urls.len() > 5 {
                println!("    {} ... and {} more", "".dimmed(), modified_urls.len() - 5);
            }
        }
    }

    Ok(())
}

fn run_submission(entry_filter: Option<&[i64]>) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize database
    let conn = db::init_db()?;
    
    // Get sources to process
    let sources = get_sources_to_process(entry_filter)?;
    
    // Validate that all sources have required config
    for source in &sources {
        if source.api_key.is_empty() || source.host.is_empty() {
            return Err(format!(
                "Source {} ({}) is missing required configuration (api_key or host). Run '{} --config' to configure.",
                source.id, source.source_url, env!("CARGO_PKG_NAME")
            ).into());
        }
    }
    
    if sources.len() > 1 {
        println!(
            "{} Processing {} sources...\n",
            "ℹ".cyan().bold(),
            sources.len()
        );
    }
    
    for (idx, source) in sources.iter().enumerate() {
        process_source(&conn, source, false)?;
        if idx < sources.len() - 1 {
            println!();
        }
    }

    Ok(())
}

fn run_unattended_submission(entry_filter: Option<&[i64]>) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize database
    let conn = db::init_db()?;
    
    // Get sources to process
    let sources = get_sources_to_process(entry_filter)?;
    
    // Validate that all sources have required config
    for source in &sources {
        if source.api_key.is_empty() || source.host.is_empty() {
            return Err(format!(
                "Source {} ({}) is missing required configuration (api_key or host). Run '{} --config' to configure.",
                source.id, source.source_url, env!("CARGO_PKG_NAME")
            ).into());
        }
    }
    
    if sources.len() > 1 {
        println!(
            "{} Processing {} sources (unattended)...\n",
            "ℹ".cyan().bold(),
            sources.len()
        );
    }
    
    for (idx, source) in sources.iter().enumerate() {
        process_source(&conn, source, true)?;
        if idx < sources.len() - 1 {
            println!();
        }
    }

    Ok(())
}

fn process_source(
    conn: &rusqlite::Connection,
    source: &db::Source,
    unattended: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let source_type = if source.source_type == "sitemap" {
        SourceType::Sitemap
    } else {
        SourceType::Feed
    };
    let source_type_str = if source.source_type == "sitemap" { "sitemap" } else { "feed" };
    
    println!(
        "{} [{}] Fetching {} from {}...",
        "→".blue().bold(),
        source.id.to_string().bold(),
        source_type_str,
        source.source_url
    );

    let entries: Vec<UrlEntry> = match source_type {
        SourceType::Feed => feed::fetch_feed_urls(&source.source_url)?,
        SourceType::Sitemap => sitemap::fetch_sitemap_urls(&source.source_url)?,
    };

    if entries.is_empty() {
        println!(
            "  {} No URLs found in {}.",
            "⚠".yellow().bold(),
            source_type_str
        );
        println!(
            "  {} Add content to your {} and run again.",
            "→".blue().bold(),
            source_type_str
        );
        return Ok(());
    }

    println!(
        "  {} Found {} URLs in {}.",
        "✓".green().bold(),
        entries.len(),
        source_type_str
    );

    // Check if this is first run for this source
    let is_first_run = db::is_source_first_run(conn, source.id)?;

    if is_first_run {
        if unattended {
            handle_first_run_unattended(conn, source, &entries)
        } else {
            handle_first_run(conn, source, &entries)
        }
    } else {
        if unattended {
            handle_subsequent_run_unattended(conn, source, &entries)
        } else {
            handle_subsequent_run(conn, source, &entries)
        }
    }
}

fn handle_first_run(
    conn: &rusqlite::Connection,
    source: &db::Source,
    entries: &[UrlEntry],
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "\n  {} First run detected for this source. Found {} URLs.",
        "ℹ".cyan().bold(),
        entries.len()
    );

    // Store all URLs in database first
    println!("  {} Storing URLs in database...", "→".blue().bold());
    for entry in entries {
        db::add_url_with_date_for_source(conn, source.id, &entry.url, entry.date.as_deref())?;
    }
    println!(
        "  {} Stored {} URLs.",
        "✓".green().bold(),
        entries.len()
    );

    // Ask user if they want to submit all URLs
    println!();
    println!(
        "  {} {}",
        "⚠ WARNING:".yellow().bold(),
        "Submitting all URLs on first run may include outdated or deprecated links."
    );

    let should_submit = Confirm::new()
        .with_prompt(format!("  Do you want to submit all {} found URLs?", entries.len()))
        .default(false)
        .interact()?;

    if should_submit {
        // Build submit entries
        let submit_entries: Vec<SubmitEntry> = entries
            .iter()
            .map(|e| SubmitEntry {
                url: e.url.clone(),
                reason: SubmitReason::New,
            })
            .collect();

        println!(
            "\n  {} Submitting {} URL(s) to {}...\n",
            "→".blue().bold(),
            submit_entries.len(),
            source.searchengine
        );

        submit::submit_in_batches(&source.api_key, &source.host, &source.searchengine, &submit_entries)?;

        println!(
            "\n  {} Successfully submitted {} URL(s).",
            "✓".green().bold(),
            submit_entries.len()
        );
    } else {
        println!(
            "\n  {} URLs stored but not submitted.",
            "ℹ".cyan().bold()
        );
        println!(
            "  {} Add new content and run again to submit only the new URLs.",
            "→".blue().bold()
        );
    }

    // Mark first run as completed for this source
    db::mark_source_first_run_completed(conn, source.id)?;

    Ok(())
}

fn handle_first_run_unattended(
    conn: &rusqlite::Connection,
    source: &db::Source,
    entries: &[UrlEntry],
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "\n  {} First run detected for this source. Found {} URLs.",
        "ℹ".cyan().bold(),
        entries.len()
    );

    // Store all URLs in database first
    println!("  {} Storing URLs in database...", "→".blue().bold());
    for entry in entries {
        db::add_url_with_date_for_source(conn, source.id, &entry.url, entry.date.as_deref())?;
    }
    println!(
        "  {} Stored {} URLs.",
        "✓".green().bold(),
        entries.len()
    );

    // Automatically submit all URLs (unattended mode)
    println!();
    println!(
        "  {} Unattended mode: Submitting all URLs on first run.",
        "ℹ".cyan().bold()
    );

    // Build submit entries
    let submit_entries: Vec<SubmitEntry> = entries
        .iter()
        .map(|e| SubmitEntry {
            url: e.url.clone(),
            reason: SubmitReason::New,
        })
        .collect();

    println!(
        "\n  {} Submitting {} URL(s) to {}...\n",
        "→".blue().bold(),
        submit_entries.len(),
        source.searchengine
    );

    submit::submit_in_batches(&source.api_key, &source.host, &source.searchengine, &submit_entries)?;

    println!(
        "\n  {} Successfully submitted {} URL(s).",
        "✓".green().bold(),
        submit_entries.len()
    );

    // Mark first run as completed for this source
    db::mark_source_first_run_completed(conn, source.id)?;

    Ok(())
}

fn handle_subsequent_run(
    conn: &rusqlite::Connection,
    source: &db::Source,
    entries: &[UrlEntry],
) -> Result<(), Box<dyn std::error::Error>> {
    // Get stored URLs with their dates for this source
    let stored_urls = db::get_urls_with_dates_for_source(conn, source.id)?;

    let mut to_submit: Vec<SubmitEntry> = Vec::new();
    let mut new_count = 0;
    let mut modified_count = 0;

    for entry in entries {
        if let Some(stored_date) = stored_urls.get(&entry.url) {
            // URL exists in database - check if it was modified
            if let Some(new_date) = &entry.date {
                let is_modified = match stored_date {
                    Some(old_date) => new_date != old_date,
                    None => true, // No previous date, treat as modified
                };

                if is_modified {
                    to_submit.push(SubmitEntry {
                        url: entry.url.clone(),
                        reason: SubmitReason::Modified {
                            date: new_date.clone(),
                        },
                    });
                    modified_count += 1;
                }
            }
        } else {
            // New URL - not in database
            to_submit.push(SubmitEntry {
                url: entry.url.clone(),
                reason: SubmitReason::New,
            });
            new_count += 1;
        }
    }

    if to_submit.is_empty() {
        println!(
            "  {} No new or modified URLs to submit. All URLs are up to date.",
            "✓".green().bold()
        );
        return Ok(());
    }

    println!(
        "\n  {} Found {} URL(s) to submit: {} new, {} modified",
        "ℹ".cyan().bold(),
        to_submit.len(),
        new_count,
        modified_count
    );

    // List URLs to be submitted
    if new_count > 0 {
        println!("\n  {} ({}):", "New URLs".green().bold(), new_count);
        for entry in to_submit.iter().filter(|e| matches!(e.reason, SubmitReason::New)).take(5) {
            println!("    • {}", entry.url);
        }
        if new_count > 5 {
            println!("    {} ... and {} more", "".dimmed(), new_count - 5);
        }
    }
    if modified_count > 0 {
        println!("\n  {} ({}):", "Modified URLs".yellow().bold(), modified_count);
        for entry in to_submit.iter().filter(|e| matches!(e.reason, SubmitReason::Modified { .. })).take(5) {
            if let SubmitReason::Modified { date } = &entry.reason {
                println!("    • {} (updated: {})", entry.url, date.cyan());
            }
        }
        if modified_count > 5 {
            println!("    {} ... and {} more", "".dimmed(), modified_count - 5);
        }
    }

    // Confirm before submitting
    println!();
    let should_submit = Confirm::new()
        .with_prompt(format!("  Submit {} URL(s) to IndexNow?", to_submit.len()))
        .default(true)
        .interact()?;

    if !should_submit {
        println!(
            "\n  {} Submission cancelled.",
            "ℹ".cyan().bold()
        );
        return Ok(());
    }

    println!(
        "\n  {} Submitting to {}...\n",
        "→".blue().bold(),
        source.searchengine
    );

    submit::submit_in_batches(&source.api_key, &source.host, &source.searchengine, &to_submit)?;

    // Update database with submitted URLs
    for entry in &to_submit {
        let date = match &entry.reason {
            SubmitReason::New => entries
                .iter()
                .find(|e| e.url == entry.url)
                .and_then(|e| e.date.as_deref()),
            SubmitReason::Modified { date } => Some(date.as_str()),
        };
        db::add_url_with_date_for_source(conn, source.id, &entry.url, date)?;
    }

    println!(
        "\n  {} Successfully submitted and stored {} URL(s).",
        "✓".green().bold(),
        to_submit.len()
    );

    Ok(())
}

fn handle_subsequent_run_unattended(
    conn: &rusqlite::Connection,
    source: &db::Source,
    entries: &[UrlEntry],
) -> Result<(), Box<dyn std::error::Error>> {
    // Get stored URLs with their dates for this source
    let stored_urls = db::get_urls_with_dates_for_source(conn, source.id)?;

    let mut to_submit: Vec<SubmitEntry> = Vec::new();
    let mut new_count = 0;
    let mut modified_count = 0;

    for entry in entries {
        if let Some(stored_date) = stored_urls.get(&entry.url) {
            // URL exists in database - check if it was modified
            if let Some(new_date) = &entry.date {
                let is_modified = match stored_date {
                    Some(old_date) => new_date != old_date,
                    None => true, // No previous date, treat as modified
                };

                if is_modified {
                    to_submit.push(SubmitEntry {
                        url: entry.url.clone(),
                        reason: SubmitReason::Modified {
                            date: new_date.clone(),
                        },
                    });
                    modified_count += 1;
                }
            }
        } else {
            // New URL - not in database
            to_submit.push(SubmitEntry {
                url: entry.url.clone(),
                reason: SubmitReason::New,
            });
            new_count += 1;
        }
    }

    if to_submit.is_empty() {
        println!(
            "  {} No new or modified URLs to submit. All URLs are up to date.",
            "✓".green().bold()
        );
        return Ok(());
    }

    println!(
        "\n  {} Found {} URL(s) to submit: {} new, {} modified",
        "ℹ".cyan().bold(),
        to_submit.len(),
        new_count,
        modified_count
    );

    // List URLs to be submitted
    if new_count > 0 {
        println!("\n  {} ({}):", "New URLs".green().bold(), new_count);
        for entry in to_submit.iter().filter(|e| matches!(e.reason, SubmitReason::New)).take(5) {
            println!("    • {}", entry.url);
        }
        if new_count > 5 {
            println!("    {} ... and {} more", "".dimmed(), new_count - 5);
        }
    }
    if modified_count > 0 {
        println!("\n  {} ({}):", "Modified URLs".yellow().bold(), modified_count);
        for entry in to_submit.iter().filter(|e| matches!(e.reason, SubmitReason::Modified { .. })).take(5) {
            if let SubmitReason::Modified { date } = &entry.reason {
                println!("    • {} (updated: {})", entry.url, date.cyan());
            }
        }
        if modified_count > 5 {
            println!("    {} ... and {} more", "".dimmed(), modified_count - 5);
        }
    }

    // Unattended mode: submit without confirmation
    println!();
    println!(
        "  {} Unattended mode: Submitting {} URL(s) to {}...\n",
        "→".blue().bold(),
        to_submit.len(),
        source.searchengine
    );

    submit::submit_in_batches(&source.api_key, &source.host, &source.searchengine, &to_submit)?;

    // Update database with submitted URLs
    for entry in &to_submit {
        let date = match &entry.reason {
            SubmitReason::New => entries
                .iter()
                .find(|e| e.url == entry.url)
                .and_then(|e| e.date.as_deref()),
            SubmitReason::Modified { date } => Some(date.as_str()),
        };
        db::add_url_with_date_for_source(conn, source.id, &entry.url, date)?;
    }

    println!(
        "\n  {} Successfully submitted and stored {} URL(s).",
        "✓".green().bold(),
        to_submit.len()
    );

    Ok(())
}
