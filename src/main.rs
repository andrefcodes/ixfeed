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

use clap::{Parser, Subcommand};
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
#[command(disable_help_subcommand = true)]
#[command(about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Edit configuration (API key, source URL, host, search engine)
    Config,
    /// Show current configuration
    Show,
    /// Clear the database (WARNING: destructive operation)
    ClearDb,
    /// Dry run - show URLs that would be submitted without actually submitting
    DryRun,
    /// Show version information
    Version,
    /// Show help information
    Help,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Config) => {
            if let Err(e) = config::edit_config() {
                eprintln!("{}: {}", "Error".red().bold(), e);
                process::exit(1);
            }
        }
        Some(Commands::Show) => {
            if let Err(e) = config::list_config() {
                eprintln!("{}: {}", "Error".red().bold(), e);
                process::exit(1);
            }
        }
        Some(Commands::ClearDb) => {
            if let Err(e) = db::clear_database() {
                eprintln!("{}: {}", "Error".red().bold(), e);
                process::exit(1);
            }
        }
        Some(Commands::Version) => {
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
        }
        Some(Commands::Help) => {
            print_help();
        }
        Some(Commands::DryRun) => {
            if let Err(e) = run_dry_run() {
                eprintln!("{}: {}", "Error".red().bold(), e);
                process::exit(1);
            }
        }
        None => {
            // Check if config is complete
            if !config::is_config_complete() {
                println!(
                    "{} No configuration found. Let's set it up first.\n",
                    "ℹ".cyan().bold()
                );
                if let Err(e) = config::edit_config() {
                    eprintln!("{}: {}", "Error".red().bold(), e);
                    process::exit(1);
                }
                // Verify config was saved properly
                if !config::is_config_complete() {
                    eprintln!(
                        "{}: Configuration incomplete. Please run '{} config' to complete setup.",
                        "Error".red().bold(),
                        env!("CARGO_PKG_NAME")
                    );
                    process::exit(1);
                }
                println!(); // Blank line before submission
            }
            // Now run the submission workflow
            if let Err(e) = run_submission() {
                eprintln!("{}: {}", "Error".red().bold(), e);
                process::exit(1);
            }
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
    println!("  {} [COMMAND]", env!("CARGO_PKG_NAME"));
    println!();
    println!("{}", "Commands:".bold());
    println!("  {}     Edit configuration (API key, source URL, host, search engine)", "config".cyan());
    println!("  {}       Show current configuration", "show".cyan());
    println!("  {}   Clear the database (WARNING: destructive operation)", "clear-db".cyan());
    println!("  {}    Dry run - show URLs that would be submitted", "dry-run".cyan());
    println!("  {}    Show version information", "version".cyan());
    println!("  {}       Show this help message", "help".cyan());
    println!();
    println!("{}", "Examples:".bold());
    println!("  {} config    # Configure API key, feed/sitemap URL, etc.", env!("CARGO_PKG_NAME"));
    println!("  {}           # Run submission (default command)", env!("CARGO_PKG_NAME"));
    println!("  {} dry-run   # Show what would be submitted", env!("CARGO_PKG_NAME"));
    println!("  {} show      # Show current configuration", env!("CARGO_PKG_NAME"));
}

fn run_dry_run() -> Result<(), Box<dyn std::error::Error>> {
    // Load and validate config
    let cfg = config::load_config()?;
    config::validate_config(&cfg)?;

    // Initialize database
    let conn = db::init_db()?;

    println!(
        "{} {} Dry Run Mode {}",
        "═".repeat(15).blue(),
        "DRY RUN".yellow().bold(),
        "═".repeat(15).blue()
    );
    println!("{}", "No URLs will be submitted.\n".dimmed());

    // Fetch URLs from source
    let source_type_str = match cfg.source_type {
        SourceType::Feed => "feed",
        SourceType::Sitemap => "sitemap",
    };
    println!(
        "{} Fetching {} from {}...",
        "→".blue().bold(),
        source_type_str,
        cfg.source_url
    );

    let entries: Vec<UrlEntry> = match cfg.source_type {
        SourceType::Feed => feed::fetch_feed_urls(&cfg.source_url)?,
        SourceType::Sitemap => sitemap::fetch_sitemap_urls(&cfg.source_url)?,
    };

    if entries.is_empty() {
        println!(
            "\n{} No URLs found in {}.",
            "⚠".yellow().bold(),
            source_type_str
        );
        return Ok(());
    }

    // Check if first run
    let is_first_run = db::is_first_run(&conn)?;

    if is_first_run {
        println!(
            "\n{} First run detected. {} URL(s) found:\n",
            "ℹ".cyan().bold(),
            entries.len()
        );
        
        for (i, entry) in entries.iter().enumerate() {
            let date_str = entry.date.as_deref().unwrap_or("no date");
            println!(
                "  {}. {} {}",
                (i + 1).to_string().dimmed(),
                entry.url.green(),
                format!("({})", date_str).dimmed()
            );
        }

        println!(
            "\n{} On actual run, you would be asked to confirm submission of all {} URL(s).",
            "ℹ".cyan().bold(),
            entries.len()
        );
    } else {
        // Check for new or modified URLs
        let stored_urls = db::get_urls_with_dates(&conn)?;
        
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
                "\n{} No new or modified URLs to submit.",
                "✓".green().bold()
            );
            println!(
                "{} All {} URL(s) are already up to date.",
                "ℹ".cyan().bold(),
                entries.len()
            );
            return Ok(());
        }

        println!(
            "\n{} Would submit {} URL(s):\n",
            "ℹ".cyan().bold(),
            total_to_submit
        );

        if !new_urls.is_empty() {
            println!("{} ({}):", "New URLs".green().bold(), new_urls.len());
            for (i, entry) in new_urls.iter().enumerate() {
                let date_str = entry.date.as_deref().unwrap_or("no date");
                println!(
                    "  {}. {} {}",
                    (i + 1).to_string().dimmed(),
                    entry.url,
                    format!("({})", date_str).dimmed()
                );
            }
        }

        if !modified_urls.is_empty() {
            if !new_urls.is_empty() {
                println!();
            }
            println!("{} ({}):", "Modified URLs".yellow().bold(), modified_urls.len());
            for (i, (entry, old_date)) in modified_urls.iter().enumerate() {
                let old_str = old_date.as_deref().unwrap_or("unknown");
                let new_str = entry.date.as_deref().unwrap_or("unknown");
                println!(
                    "  {}. {} {} → {}",
                    (i + 1).to_string().dimmed(),
                    entry.url,
                    old_str.dimmed(),
                    new_str.cyan()
                );
            }
        }
    }

    println!(
        "\n{} To actually submit, run: {}",
        "→".blue().bold(),
        env!("CARGO_PKG_NAME").cyan()
    );

    Ok(())
}

fn run_submission() -> Result<(), Box<dyn std::error::Error>> {
    // Load and validate config
    let cfg = config::load_config()?;
    config::validate_config(&cfg)?;

    // Initialize database
    let conn = db::init_db()?;

    // Fetch URLs from source
    let source_type_str = match cfg.source_type {
        SourceType::Feed => "feed",
        SourceType::Sitemap => "sitemap",
    };
    println!(
        "{} Fetching {} from {}...",
        "→".blue().bold(),
        source_type_str,
        cfg.source_url
    );

    let entries: Vec<UrlEntry> = match cfg.source_type {
        SourceType::Feed => feed::fetch_feed_urls(&cfg.source_url)?,
        SourceType::Sitemap => sitemap::fetch_sitemap_urls(&cfg.source_url)?,
    };

    if entries.is_empty() {
        println!(
            "\n{} No URLs found in {}.",
            "⚠".yellow().bold(),
            source_type_str
        );
        println!(
            "{} Add content to your {} and run again.",
            "→".blue().bold(),
            source_type_str
        );
        return Ok(());
    }

    println!(
        "{} Found {} URLs in {}.",
        "✓".green().bold(),
        entries.len(),
        source_type_str
    );

    // Check if this is first run using database flag
    let is_first_run = db::is_first_run(&conn)?;

    if is_first_run {
        return handle_first_run(&conn, &cfg, &entries);
    }

    // Subsequent runs: check for new and modified URLs
    handle_subsequent_run(&conn, &cfg, &entries)
}

fn handle_first_run(
    conn: &rusqlite::Connection,
    cfg: &config::Config,
    entries: &[UrlEntry],
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "\n{} First run detected. Found {} URLs.",
        "ℹ".cyan().bold(),
        entries.len()
    );

    // Store all URLs in database first
    println!("{} Storing URLs in database...", "→".blue().bold());
    for entry in entries {
        db::add_url_with_date(conn, &entry.url, entry.date.as_deref())?;
    }
    println!(
        "{} Stored {} URLs.",
        "✓".green().bold(),
        entries.len()
    );

    // Ask user if they want to submit all URLs
    println!();
    println!(
        "{} {}",
        "⚠ WARNING:".yellow().bold(),
        "Submitting all URLs on first run may include outdated or deprecated links."
    );

    let should_submit = Confirm::new()
        .with_prompt("Do you want to submit all found URLs?")
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
            "\n{} Submitting {} URL(s) to {}...\n",
            "→".blue().bold(),
            submit_entries.len(),
            cfg.searchengine
        );

        submit::submit_in_batches(cfg, &submit_entries)?;

        println!(
            "\n{} Successfully submitted {} URL(s).",
            "✓".green().bold(),
            submit_entries.len()
        );
    } else {
        println!(
            "\n{} URLs stored but not submitted.",
            "ℹ".cyan().bold()
        );
        println!(
            "{} Add new content and run again to submit only the new URLs.",
            "→".blue().bold()
        );
    }

    // Mark first run as completed
    db::mark_first_run_completed(conn)?;

    Ok(())
}

fn handle_subsequent_run(
    conn: &rusqlite::Connection,
    cfg: &config::Config,
    entries: &[UrlEntry],
) -> Result<(), Box<dyn std::error::Error>> {
    // Get stored URLs with their dates
    let stored_urls = db::get_urls_with_dates(conn)?;

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
            "{} No new or modified URLs to submit. All URLs are up to date.",
            "✓".green().bold()
        );
        return Ok(());
    }

    println!(
        "\n{} Found {} URL(s) to submit: {} new, {} modified",
        "ℹ".cyan().bold(),
        to_submit.len(),
        new_count,
        modified_count
    );

    // List URLs to be submitted
    if new_count > 0 {
        println!("\n{} ({}):", "New URLs".green().bold(), new_count);
        for entry in to_submit.iter().filter(|e| matches!(e.reason, SubmitReason::New)) {
            println!("  • {}", entry.url);
        }
    }
    if modified_count > 0 {
        println!("\n{} ({}):", "Modified URLs".yellow().bold(), modified_count);
        for entry in &to_submit {
            if let SubmitReason::Modified { date } = &entry.reason {
                println!("  • {} (updated: {})", entry.url, date.cyan());
            }
        }
    }

    // Confirm before submitting
    println!();
    let should_submit = Confirm::new()
        .with_prompt(format!("Submit {} URL(s) to IndexNow?", to_submit.len()))
        .default(true)
        .interact()?;

    if !should_submit {
        println!(
            "\n{} Submission cancelled.",
            "ℹ".cyan().bold()
        );
        return Ok(());
    }

    println!(
        "\n{} Submitting to {}...\n",
        "→".blue().bold(),
        cfg.searchengine
    );

    submit::submit_in_batches(cfg, &to_submit)?;

    // Update database with submitted URLs
    for entry in &to_submit {
        let date = match &entry.reason {
            SubmitReason::New => entries
                .iter()
                .find(|e| e.url == entry.url)
                .and_then(|e| e.date.as_deref()),
            SubmitReason::Modified { date } => Some(date.as_str()),
        };
        db::add_url_with_date(conn, &entry.url, date)?;
    }

    println!(
        "\n{} Successfully submitted and stored {} URL(s).",
        "✓".green().bold(),
        to_submit.len()
    );

    Ok(())
}
