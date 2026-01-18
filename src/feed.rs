//! Feed parsing for RSS, Atom, and JSON feeds

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

use feed_rs::parser;
use reqwest::blocking::Client;
use std::time::Duration;

/// Represents a URL entry with its associated date
#[derive(Debug, Clone)]
pub struct UrlEntry {
    pub url: String,
    /// For Atom feeds: uses `updated` if available, falls back to `published`
    /// For RSS/JSON feeds: uses `published` date
    /// For Sitemaps: uses `lastmod` date
    pub date: Option<String>,
}

pub fn fetch_feed_urls(feed_url: &str) -> Result<Vec<UrlEntry>, Box<dyn std::error::Error>> {
    let user_agent = format!(
        "{}/{} (+{})",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_REPOSITORY")
    );

    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(user_agent)
        .build()?;

    let response = client.get(feed_url).send()?;

    if !response.status().is_success() {
        return Err(format!(
            "Failed to fetch feed: HTTP {}",
            response.status()
        )
        .into());
    }

    let content = response.bytes()?;

    // feed-rs automatically detects RSS, Atom, or JSON Feed format
    let feed = parser::parse(&content[..])?;

    let entries: Vec<UrlEntry> = feed
        .entries
        .into_iter()
        .filter_map(|entry| {
            // Try to get the link from the entry
            let url = entry
                .links
                .first()
                .map(|link| link.href.clone())
                .or_else(|| entry.id.parse().ok())?;

            // Get the best date for modification tracking:
            // - For Atom: prefer `updated` over `published` (updated = content changed)
            // - For RSS: use `published` (RSS doesn't have an updated field)
            // - For JSON Feed: use `date_modified` if available, else `date_published`
            let date = entry
                .updated
                .map(|dt| dt.to_rfc3339())
                .or_else(|| entry.published.map(|dt| dt.to_rfc3339()));

            Some(UrlEntry { url, date })
        })
        .collect();

    Ok(entries)
}
