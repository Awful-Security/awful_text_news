//! Output generation modules for JSON, Markdown, and index files.
//!
//! This module contains submodules responsible for writing processed articles
//! to various output formats:
//!
//! # Submodules
//!
//! - [`json`]: Writes `FrontPage` data to JSON files for API consumption
//! - [`markdown`]: Converts `FrontPage` to Markdown format for reading
//! - [`indexes`]: Updates various index files for navigation (TOC, SUMMARY.md, etc.)
//!
//! # Output Structure
//!
//! ```text
//! json_output_dir/
//! ├── 2025-05-06/
//! │   ├── morning.json
//! │   ├── afternoon.json
//! │   └── evening.json
//!
//! markdown_output_dir/
//! ├── 2025-05-06.md          # Date TOC
//! ├── 2025-05-06_morning.md  # Full edition
//! ├── daily_news.md          # Master index
//! └── SUMMARY.md             # mdBook navigation
//! ```

pub mod indexes;
pub mod json;
pub mod markdown;
