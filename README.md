# ixfeed

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)

A Rust CLI tool that watches RSS/Atom/JSON feeds and sitemaps, then submits new or modified URLs to IndexNow-compatible search engines (Bing, Yandex, and others).

## Features

- **Multi-format support**: RSS, Atom, JSON Feed, and Sitemap XML (with recursive sitemap index support)
- **Smart tracking**: SQLite database tracks submitted URLs and modification dates
- **Modification detection**: Re-submits URLs when content is updated (using `lastmod`, `updated`, or `published` dates)
- **First-run safety**: On first run, stores URLs and asks for confirmation before submitting
- **Bulk submission**: Supports IndexNow bulk API (up to 10,000 URLs per batch)
- **Dry-run mode**: Preview what would be submitted without making changes
- **Auto URL validation**: Validates feed/sitemap URLs, auto-upgrades HTTP to HTTPS
- **Color-coded output**: Clear visual feedback for success/error states

## Installation

### Pre-built Binaries

Download the latest release for your platform from the [Releases page](https://github.com/andrefcodes/ixfeed/releases).

Available for:
- Linux (x86_64, aarch64)
- macOS (x86_64, Apple Silicon)
- Windows (x86_64)

### Build from Source

```bash
cd ixfeed
cargo build --release

# The binary will be at ./target/release/ixfeed
```

## Quick Start

1. **Generate an IndexNow key**: Create a random string (e.g., `openssl rand -hex 16`)

2. **Upload key file**: Save your key to `https://yourdomain.com/{key}.txt`  
   The file should contain only the key value.

3. **Configure and run**:
   ```bash
   ixfeed
   ```
   On first run, you'll be prompted to configure API key, feed/sitemap URL, and search engine.

4. **Subsequent runs** (submits new/modified URLs):
   ```bash
   ixfeed
   ```

## Commands

| Command | Description |
|---------|-------------|
| `ixfeed` | Run the submission process (default) |
| `ixfeed config` | Edit configuration interactively |
| `ixfeed show` | Show current configuration |
| `ixfeed dry-run` | Preview URLs that would be submitted |
| `ixfeed unattended` | Submit URLs without confirmation (for automation) |
| `ixfeed clear-db` | Clear the URL database (destructive!) |
| `ixfeed version` | Show version |
| `ixfeed help` | Show help |

> **⚠️ Important**: Run `ixfeed` (interactive mode) at least once before using `ixfeed unattended`. The first run in unattended mode will automatically submit all URLs from your feed/sitemap, which may include outdated or deprecated content.

## Configuration

Configuration is stored in SQLite database:
- **Linux**: `~/.local/share/ixfeed/ixfeed.db`
- **macOS**: `~/Library/Application Support/ixfeed/ixfeed.db`
- **Windows**: `%APPDATA%\ixfeed\ixfeed.db`

### Settings

| Setting | Description | Example |
|---------|-------------|---------|
| `api_key` | Your IndexNow API key | `a1b2c3d4e5f6...` |
| `source_type` | Feed or Sitemap | `feed` or `sitemap` |
| `source_url` | URL to your feed or sitemap | `https://example.com/sitemap.xml` |
| `host` | Your domain (auto-detected from URL) | `example.com` |
| `searchengine` | IndexNow endpoint | `api.indexnow.org` |

### IndexNow Endpoints

| Endpoint | Notes |
|----------|-------|
| `api.indexnow.org` | **Recommended** - forwards to all participating engines |
| `www.bing.com` | Bing directly |
| `yandex.com` | Yandex directly |
| `search.seznam.cz` | Seznam directly |

## Response Codes

| Code | Meaning | Action |
|------|---------|--------|
| 200/202 | ✅ Success | URL accepted for indexing |
| 400 | ❌ Bad Request | Check URL format |
| 401 | ❌ Unauthorized | Verify API key |
| 403 | ❌ Forbidden | Check key file at `https://{host}/{key}.txt` |
| 422 | ❌ Unprocessable | URLs must match configured host |
| 429 | ⚠️ Rate Limited | Wait and retry later |

## Workflow

```
┌─────────────────────────────────────────────────────────┐
│                     First Run                           │
├─────────────────────────────────────────────────────────┤
│ 1. Fetch feed/sitemap URLs                              │
│ 2. Store all URLs in database                           │
│ 3. Ask user if they want to submit (default: No)        │
│    OR: Submit automatically (unattended mode)           │
│ 4. Mark first run as completed                          │
└─────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────┐
│                   Subsequent Runs                       │
├─────────────────────────────────────────────────────────┤
│ 1. Fetch feed/sitemap URLs                              │
│ 2. Compare with stored URLs and dates                   │
│ 3. Identify NEW and MODIFIED URLs                       │
│ 4. Ask for confirmation (default: Yes)                  │
│    OR: Submit automatically (unattended mode)           │
│ 5. Submit to IndexNow and update database               │
└─────────────────────────────────────────────────────────┘
```

## Automation

> **⚠️ Warning**: Before setting up automated runs with `ixfeed unattended`, run the application interactively (`ixfeed`) at least once to review and confirm the initial URL submission. Unattended mode will automatically submit all URLs on first run without confirmation.

### Cron (Linux/macOS)

```bash
# Run every hour (use unattended subcommand for non-interactive)
0 * * * * /path/to/ixfeed unattended >> /var/log/ixfeed.log 2>&1
```

### Systemd Timer

Create `/etc/systemd/system/ixfeed.service`:
```ini
[Unit]
Description=IndexNow Feed/Sitemap Submitter

[Service]
Type=oneshot
ExecStart=/path/to/ixfeed unattended
User=youruser
```

Create `/etc/systemd/system/ixfeed.timer`:
```ini
[Unit]
Description=Run IndexNow submitter hourly

[Timer]
OnCalendar=hourly
Persistent=true

[Install]
WantedBy=timers.target
```

Enable with:
```bash
sudo systemctl enable --now ixfeed.timer
```

## License

AGPL-3.0-or-later – see [LICENSE](LICENSE) for details.
