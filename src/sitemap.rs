//! Sitemap parsing with recursive sitemap index support

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

use crate::feed::UrlEntry;
use colored::*;
use regex::Regex;
use reqwest::blocking::Client;
use std::collections::HashSet;
use std::time::Duration;

/// Fetch all URLs from a sitemap, recursively handling sitemap indexes
pub fn fetch_sitemap_urls(sitemap_url: &str) -> Result<Vec<UrlEntry>, Box<dyn std::error::Error>> {
    let client = build_client()?;
    let mut seen_urls: HashSet<String> = HashSet::new();
    let mut entries: Vec<UrlEntry> = Vec::new();

    fetch_sitemap_recursive(&client, sitemap_url, &mut entries, &mut seen_urls, 0)?;

    Ok(entries)
}

fn build_client() -> Result<Client, Box<dyn std::error::Error>> {
    let user_agent = format!(
        "{}/{} (+{})",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_REPOSITORY")
    );

    Ok(Client::builder()
        .timeout(Duration::from_secs(60))
        .user_agent(user_agent)
        .build()?)
}

fn fetch_sitemap_recursive(
    client: &Client,
    url: &str,
    entries: &mut Vec<UrlEntry>,
    seen_urls: &mut HashSet<String>,
    depth: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    // Prevent infinite recursion
    const MAX_DEPTH: usize = 10;
    if depth > MAX_DEPTH {
        println!(
            "  {} Maximum sitemap depth ({}) reached, skipping: {}",
            "⚠".yellow(),
            MAX_DEPTH,
            url
        );
        return Ok(());
    }

    println!(
        "  {} Fetching sitemap: {}",
        "→".blue(),
        url.dimmed()
    );

    let response = client.get(url).send()?;

    if !response.status().is_success() {
        return Err(format!("Failed to fetch sitemap: HTTP {}", response.status()).into());
    }

    let content = response.text()?;

    // Detect if this is a sitemap index or a regular sitemap
    if content.contains("<sitemapindex") {
        // This is a sitemap index - parse and recurse
        let sub_sitemaps = parse_sitemap_index(&content)?;
        println!(
            "    {} Found sitemap index with {} sub-sitemaps",
            "ℹ".cyan(),
            sub_sitemaps.len()
        );

        for sub_url in sub_sitemaps {
            fetch_sitemap_recursive(client, &sub_url, entries, seen_urls, depth + 1)?;
        }
    } else {
        // This is a regular sitemap - parse URLs
        let urls = parse_sitemap(&content)?;
        let mut added = 0;

        for entry in urls {
            if seen_urls.insert(entry.url.clone()) {
                entries.push(entry);
                added += 1;
            }
        }

        println!(
            "    {} Found {} URLs (added {}, {} duplicates skipped)",
            "✓".green(),
            added + (entries.len() - added),
            added,
            entries.len() - added
        );
    }

    Ok(())
}

/// Parse a sitemap index XML and return the list of sitemap URLs
fn parse_sitemap_index(content: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut sitemaps = Vec::new();

    // Use regex to find <sitemap>...<loc>URL</loc>...</sitemap> blocks
    // The (?s) flag makes . match newlines
    let sitemap_re = Regex::new(r"(?s)<sitemap[^>]*>.*?</sitemap>")?;
    let loc_re = Regex::new(r"<loc>\s*([^<]+?)\s*</loc>")?;

    for sitemap_match in sitemap_re.find_iter(content) {
        let sitemap_block = sitemap_match.as_str();
        if let Some(caps) = loc_re.captures(sitemap_block) {
            if let Some(loc) = caps.get(1) {
                let url = loc.as_str().trim().to_string();
                if !url.is_empty() {
                    sitemaps.push(url);
                }
            }
        }
    }

    Ok(sitemaps)
}

/// Parse a sitemap XML and return URL entries with lastmod dates
fn parse_sitemap(content: &str) -> Result<Vec<UrlEntry>, Box<dyn std::error::Error>> {
    let mut entries = Vec::new();

    // Use regex to find <url>...<loc>URL</loc>...</url> blocks
    // The (?s) flag makes . match newlines
    let url_re = Regex::new(r"(?s)<url[^>]*>.*?</url>")?;
    let loc_re = Regex::new(r"<loc>\s*([^<]+?)\s*</loc>")?;
    let lastmod_re = Regex::new(r"<lastmod>\s*([^<]+?)\s*</lastmod>")?;

    for url_match in url_re.find_iter(content) {
        let url_block = url_match.as_str();
        
        if let Some(loc_caps) = loc_re.captures(url_block) {
            if let Some(loc) = loc_caps.get(1) {
                let url = loc.as_str().trim().to_string();
                if !url.is_empty() {
                    let lastmod = lastmod_re
                        .captures(url_block)
                        .and_then(|c| c.get(1))
                        .map(|m| m.as_str().trim().to_string());

                    entries.push(UrlEntry {
                        url,
                        date: lastmod,
                    });
                }
            }
        }
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sitemap() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url>
    <loc>https://example.com/page1</loc>
    <lastmod>2026-01-15</lastmod>
  </url>
  <url>
    <loc>https://example.com/page2</loc>
  </url>
</urlset>"#;

        let entries = parse_sitemap(xml).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].url, "https://example.com/page1");
        assert_eq!(entries[0].date, Some("2026-01-15".to_string()));
        assert_eq!(entries[1].url, "https://example.com/page2");
        assert_eq!(entries[1].date, None);
    }

    #[test]
    fn test_parse_sitemap_index() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <sitemap>
    <loc>https://example.com/posts-sitemap.xml</loc>
  </sitemap>
  <sitemap>
    <loc>https://example.com/pages-sitemap.xml</loc>
  </sitemap>
</sitemapindex>"#;

        let sitemaps = parse_sitemap_index(xml).unwrap();
        assert_eq!(sitemaps.len(), 2);
        assert_eq!(sitemaps[0], "https://example.com/posts-sitemap.xml");
        assert_eq!(sitemaps[1], "https://example.com/pages-sitemap.xml");
    }
}
