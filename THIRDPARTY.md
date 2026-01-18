# Third-Party Dependencies

This document lists all third-party dependencies used in ixfeed and their licensing information.

## Overview

ixfeed uses several third-party Rust crates to provide feed parsing, HTTP client functionality, database management, and command-line interface capabilities. All dependencies are compatible with our AGPL-3.0-or-later license.

## Direct Dependencies

### Feed Processing

#### feed-rs (v2)
- **License**: MIT OR Apache-2.0
- **Purpose**: Core feed parsing library supporting RSS, Atom, and JSON Feed formats
- **Homepage**: https://github.com/feed-rs/feed-rs

### HTTP Client

#### reqwest (v0.13)
- **License**: MIT OR Apache-2.0
- **Purpose**: HTTP client for fetching feeds and submitting to IndexNow API
- **Homepage**: https://github.com/seanmonstar/reqwest

### Database

#### rusqlite (v0.38)
- **License**: MIT
- **Purpose**: SQLite database interface for storing submitted URLs and configuration
- **Homepage**: https://github.com/rusqlite/rusqlite

### Command-Line Interface

#### clap (v4)
- **License**: MIT OR Apache-2.0
- **Purpose**: Command-line argument parsing with derive macro support
- **Homepage**: https://github.com/clap-rs/clap

#### dialoguer (v0.12)
- **License**: MIT
- **Purpose**: Interactive command-line prompts for configuration
- **Homepage**: https://github.com/console-rs/dialoguer

#### colored (v3)
- **License**: MPL-2.0
- **Purpose**: Terminal text coloring for better user experience
- **Homepage**: https://github.com/colored-rs/colored

### Utilities

#### serde (v1)
- **License**: MIT OR Apache-2.0
- **Purpose**: Serialization and deserialization framework for configuration and API payloads
- **Homepage**: https://github.com/serde-rs/serde

#### serde_json (v1)
- **License**: MIT OR Apache-2.0
- **Purpose**: JSON serialization for IndexNow API requests
- **Homepage**: https://github.com/serde-rs/json

#### dirs (v6)
- **License**: MIT OR Apache-2.0
- **Purpose**: Cross-platform system directory paths for database storage
- **Homepage**: https://github.com/soc/dirs-rs

#### url (v2)
- **License**: MIT OR Apache-2.0
- **Purpose**: URL parsing and manipulation for validation and host extraction
- **Homepage**: https://github.com/servo/rust-url

#### urlencoding (v2)
- **License**: MIT
- **Purpose**: URL encoding for API parameters
- **Homepage**: https://github.com/chifflier/urlencoding

#### regex (v1)
- **License**: MIT OR Apache-2.0
- **Purpose**: Regular expressions for sitemap XML parsing
- **Homepage**: https://github.com/rust-lang/regex

## License Compatibility

All dependencies use permissive licenses (MIT, Apache-2.0, or MPL-2.0) that are compatible with our AGPL-3.0-or-later license. These licenses allow:

- Commercial use
- Modification
- Distribution
- Private use

The MIT, Apache-2.0, and MPL-2.0 licenses are permissive and place minimal restrictions on the use of the software, making them compatible with the AGPL-3.0-or-later license used by ixfeed.

## Acknowledgments

I gratefully acknowledge the contributions of all the maintainers and contributors of these open-source projects. Their work enables ixfeed to provide reliable feed monitoring and IndexNow submission capabilities.

---

*This document was generated based on the direct dependencies listed in Cargo.toml. For a complete list including transitive dependencies, run `cargo tree` in the project directory.*
