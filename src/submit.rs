//! IndexNow submission logic

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

use crate::config::Config;
use colored::*;
use reqwest::blocking::Client;
use serde::Serialize;
use std::time::Duration;

/// Maximum URLs per bulk submission (IndexNow limit is 10,000)
pub const MAX_BATCH_SIZE: usize = 10_000;

#[derive(Serialize)]
struct BulkRequest<'a> {
    host: &'a str,
    key: &'a str,
    #[serde(rename = "urlList")]
    url_list: &'a [String],
}

/// URL with optional reason for submission (new or modified)
#[derive(Debug, Clone)]
pub struct SubmitEntry {
    pub url: String,
    pub reason: SubmitReason,
}

#[derive(Debug, Clone)]
pub enum SubmitReason {
    New,
    Modified { date: String },
}

impl std::fmt::Display for SubmitReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubmitReason::New => write!(f, "new"),
            SubmitReason::Modified { date } => write!(f, "modified on {}", date),
        }
    }
}

pub fn submit_single(cfg: &Config, entry: &SubmitEntry) -> Result<(), Box<dyn std::error::Error>> {
    let client = build_client()?;

    let submit_url = format!(
        "https://{}/indexnow?url={}&key={}",
        cfg.searchengine,
        urlencoding::encode(&entry.url),
        cfg.api_key
    );

    print_url_info(entry);

    let response = client.get(&submit_url).send()?;
    let status = response.status();

    print_status_response(status.as_u16(), &entry.url)?;

    if !status.is_success() && status.as_u16() != 202 {
        return Err(format!("Submission failed with status {}", status).into());
    }

    Ok(())
}

/// Submit URLs in batches of up to MAX_BATCH_SIZE
pub fn submit_in_batches(cfg: &Config, entries: &[SubmitEntry]) -> Result<(), Box<dyn std::error::Error>> {
    let total = entries.len();
    let num_batches = (total + MAX_BATCH_SIZE - 1) / MAX_BATCH_SIZE;

    if num_batches > 1 {
        println!(
            "{} Submitting {} URLs in {} batches (max {} per batch)",
            "ℹ".cyan().bold(),
            total,
            num_batches,
            MAX_BATCH_SIZE
        );
    }

    for (batch_idx, chunk) in entries.chunks(MAX_BATCH_SIZE).enumerate() {
        if num_batches > 1 {
            println!(
                "\n{} Batch {}/{} ({} URLs)",
                "→".blue().bold(),
                batch_idx + 1,
                num_batches,
                chunk.len()
            );
        }

        if chunk.len() == 1 {
            submit_single(cfg, &chunk[0])?;
        } else {
            submit_bulk(cfg, chunk)?;
        }
    }

    Ok(())
}

fn submit_bulk(cfg: &Config, entries: &[SubmitEntry]) -> Result<(), Box<dyn std::error::Error>> {
    let client = build_client()?;

    let submit_url = format!("https://{}/indexnow", cfg.searchengine);

    let urls: Vec<String> = entries.iter().map(|e| e.url.clone()).collect();

    let payload = BulkRequest {
        host: &cfg.host,
        key: &cfg.api_key,
        url_list: &urls,
    };

    println!("  {} (bulk submission of {} URLs)", "URLs:".bold(), entries.len());
    for entry in entries {
        print_url_info(entry);
    }
    println!();

    let response = client
        .post(&submit_url)
        .header("Content-Type", "application/json; charset=utf-8")
        .json(&payload)
        .send()?;

    let status = response.status();

    print_status_response(status.as_u16(), "bulk submission")?;

    if !status.is_success() && status.as_u16() != 202 {
        return Err(format!("Submission failed with status {}", status).into());
    }

    Ok(())
}

fn print_url_info(entry: &SubmitEntry) {
    match &entry.reason {
        SubmitReason::New => {
            println!("    {} {} {}", "•".green(), entry.url, "(new)".green());
        }
        SubmitReason::Modified { date } => {
            println!(
                "    {} {} {}",
                "•".yellow(),
                entry.url,
                format!("(modified on {})", date).yellow()
            );
        }
    }
}

fn build_client() -> Result<Client, Box<dyn std::error::Error>> {
    let user_agent = format!(
        "{}/{} (+{})",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_REPOSITORY")
    );

    Ok(Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(user_agent)
        .build()?)
}

fn print_status_response(status: u16, context: &str) -> Result<(), Box<dyn std::error::Error>> {
    match status {
        200 => {
            println!(
                "  {} {} - Submission successful.",
                "200 OK".green().bold(),
                context
            );
        }
        202 => {
            println!(
                "  {} {} - Accepted, URL received.",
                "202 Accepted".green().bold(),
                context
            );
        }
        400 => {
            println!(
                "  {} {} - Invalid format or malformed request.",
                "400 Bad Request".red().bold(),
                context
            );
            print_help_400();
            return Err("Bad Request".into());
        }
        401 => {
            println!(
                "  {} {} - Invalid or missing API key.",
                "401 Unauthorized".red().bold(),
                context
            );
            print_help_401();
            return Err("Unauthorized".into());
        }
        403 => {
            println!(
                "  {} {} - Key mismatch or invalid host.",
                "403 Forbidden".red().bold(),
                context
            );
            print_help_403();
            return Err("Forbidden".into());
        }
        422 => {
            println!(
                "  {} {} - URLs don't belong to the host or key mismatch.",
                "422 Unprocessable Entity".red().bold(),
                context
            );
            print_help_422();
            return Err("Unprocessable Entity".into());
        }
        429 => {
            println!(
                "  {} {} - Rate limit exceeded.",
                "429 Too Many Requests".yellow().bold(),
                context
            );
            print_help_429();
            return Err("Rate limit exceeded".into());
        }
        _ => {
            println!(
                "  {} {} - Unexpected response.",
                format!("{}", status).yellow().bold(),
                context
            );
        }
    }

    Ok(())
}

fn print_help_400() {
    println!("\n{}", "How to fix:".cyan().bold());
    println!("  1. Check that your feed URLs are valid and properly formatted.");
    println!("  2. Ensure URLs use https:// or http:// scheme.");
    println!("  3. Verify your host configuration matches your domain.");
}

fn print_help_401() {
    println!("\n{}", "How to fix:".cyan().bold());
    println!("  1. Verify your API key is correct.");
    println!("  2. Make sure the key file exists at https://yourdomain.com/{{key}}.txt");
    println!("  3. The key file must contain only the key value, nothing else.");
    println!("  4. Run 'ixfeed config' to update your API key.");
}

fn print_help_403() {
    println!("\n{}", "How to fix:".cyan().bold());
    println!("  1. Ensure your API key file is accessible at https://{{host}}/{{key}}.txt");
    println!("  2. Check that the host in your config matches the URLs you're submitting.");
    println!("  3. Verify the key file contains the exact key value (no extra whitespace).");
    println!("  4. Run 'ixfeed list' to check your current configuration.");
}

fn print_help_422() {
    println!("\n{}", "How to fix:".cyan().bold());
    println!("  1. All URLs must belong to the same host specified in your config.");
    println!("  2. Check that your feed/sitemap only contains URLs from your domain.");
    println!("  3. Run 'ixfeed config' and verify the 'host' setting.");
}

fn print_help_429() {
    println!("\n{}", "How to fix:".cyan().bold());
    println!("  1. Wait some time before retrying (usually a few minutes to hours).");
    println!("  2. Consider submitting fewer URLs at once.");
    println!("  3. IndexNow has rate limits - space out your submissions.");
}
