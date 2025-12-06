//! Command-line interface definitions for Awful Text News.
//!
//! This module defines the CLI arguments and options using the `clap` crate.
//! All arguments can be provided via command-line flags or environment variables.

use clap::Parser;

/// Command-line arguments for the Awful Text News application.
///
/// This struct defines all configuration options that can be passed to the
/// application at runtime. Options include output directories, API keys,
/// and message bus configuration.
///
/// # Examples
///
/// ```sh
/// # Basic usage with required arguments
/// awful_text_news -j ./json -m ./markdown
///
/// # With NYT API key
/// awful_text_news -j ./json -m ./markdown --nyt-api-key YOUR_KEY
///
/// # With message bus enabled
/// awful_text_news -j ./json -m ./markdown --amqp-url amqp://localhost:5672
/// ```
#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct Cli {
    /// Output directory for the JSON API file
    #[arg(short, long)]
    pub json_output_dir: String,

    /// Output directory for the Markdown file
    #[arg(short, long)]
    pub markdown_output_dir: String,

    /// Optional path to config.yaml file
    #[arg(short, long)]
    pub config: Option<String>,

    /// New York Times API key
    #[arg(long, env = "NYT_API_KEY")]
    pub nyt_api_key: Option<String>,

    /// AMQP URL for message bus (optional, enables event publishing when `publish` feature is enabled)
    #[arg(long, env = "AMQP_URL")]
    pub amqp_url: Option<String>,

    /// Message bus exchange name (only used when `publish` feature is enabled)
    #[arg(long, env = "MESSAGE_BUS_EXCHANGE", default_value = "events")]
    pub message_bus_exchange: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        let cli = Cli::parse_from(&[
            "awful_text_news",
            "--json-output-dir",
            "./json",
            "--markdown-output-dir",
            "./markdown",
        ]);

        assert_eq!(cli.json_output_dir, "./json");
        assert_eq!(cli.markdown_output_dir, "./markdown");
    }

    #[test]
    fn test_cli_short_flags() {
        let cli = Cli::parse_from(&[
            "awful_text_news",
            "-j",
            "/tmp/json",
            "-m",
            "/tmp/markdown",
        ]);

        assert_eq!(cli.json_output_dir, "/tmp/json");
        assert_eq!(cli.markdown_output_dir, "/tmp/markdown");
    }
}
