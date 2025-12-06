//! News source scrapers for fetching articles from various outlets.
//!
//! This module contains submodules for scraping different news sources.
//! Each scraper follows a consistent two-phase pattern:
//!
//! 1. **Indexing**: Discover article URLs from the source's homepage or API
//! 2. **Fetching**: Download and parse article content from each URL
//!
//! # Supported Sources
//!
//! | Source | Module | Method | Notes |
//! |--------|--------|--------|-------|
//! | CNN Lite | [`cnn`] | HTML scraping | Text-only version of CNN |
//! | NPR Text | [`npr`] | HTML scraping | Text-only version of NPR |
//! | AP News | [`apnews`] | Google News search | Uses Google to find recent articles |
//! | Al Jazeera | [`aljazeera`] | HTML scraping | Multiple sections: news, climate, tech |
//! | BBC News | [`bbcnews`] | HTML scraping | Homepage articles only |
//! | New York Times | [`nyt`] | Top Stories API | Requires API key; uses proxy for content |
//!
//! # Common Patterns
//!
//! Each scraper module exports:
//! - `index_articles()`: Returns a list of article URLs
//! - `fetch_articles(urls)`: Fetches content from the URLs, returns `Vec<NewsArticle>`
//!
//! Scrapers use:
//! - Concurrent fetching with `futures::stream` for performance
//! - Graceful error handling (failed fetches are logged and skipped)
//! - Date extraction from multiple sources (JSON-LD, meta tags, etc.)

pub mod apnews;
pub mod cnn;
pub mod npr;
pub mod aljazeera;
pub mod bbcnews;
pub mod nyt;
