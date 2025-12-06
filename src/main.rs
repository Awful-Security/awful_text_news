//! # Awful Text News
//!
//! A news aggregation and summarization pipeline that scrapes articles from
//! text-only news sources, processes them through an LLM for summarization
//! and entity extraction, and outputs structured JSON and Markdown files.
//!
//! ## Features
//!
//! - Scrapes articles from multiple news sources (CNN Lite, NPR Text, AP News,
//!   Al Jazeera, BBC News, and New York Times)
//! - Processes articles through an OpenAI-compatible LLM API for summarization
//! - Extracts named entities, key takeaways, important dates, and timeframes
//! - Outputs JSON API files and Markdown documents for mdBook integration
//! - Supports optional event publishing via RabbitMQ message bus
//!
//! ## Usage
//!
//! ```sh
//! awful_text_news -j ./json -m ./markdown
//! ```
//!
//! ## Architecture
//!
//! The application follows a pipeline architecture:
//! 1. **Indexing**: Discover article URLs from each news source
//! 2. **Fetching**: Download article content from discovered URLs
//! 3. **Processing**: Send articles to LLM for summarization (parallel, 12 at a time)
//! 4. **Output**: Write JSON API files and Markdown reports

use awful_aj::{config, config_dir, template};
use awful_publish::BusConfig;
use chrono::Local;
use clap::Parser;
use itertools::Itertools;
use std::error::Error;
use tracing::{debug, error, info, instrument, warn};
use tracing_subscriber::{fmt as tfmt, EnvFilter};

mod api;
mod cli;
mod models;
mod outputs;
mod scrapers;
mod utils;

use api::ask_with_backoff;
use cli::Cli;
use models::{AwfulNewsArticle, FrontPage, ImportantDate, ImportantTimeframe, NamedEntity};
use outputs::{indexes, json, markdown};
use utils::{ensure_writable_dir, looks_truncated, time_of_day, truncate_for_log};

#[tokio::main]
#[instrument]
async fn main() -> Result<(), Box<dyn Error>> {
    // --- Tracing init ---
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tfmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_file(false)
        .with_line_number(false)
        .with_timer(tracing_subscriber::fmt::time::UtcTime::rfc_3339())
        .init();

    let start_time = std::time::Instant::now();
    info!("news_update starting up");

    // Parse CLI
    let args = Cli::parse();
    debug!(?args.json_output_dir, ?args.markdown_output_dir, "Parsed CLI arguments");

    // --- Initialize message bus (if configured) ---
    if let Some(ref amqp_url) = args.amqp_url {
        let bus_config = BusConfig::new(amqp_url.clone(), args.message_bus_exchange.clone());
        if let Err(e) = awful_publish::init_global(bus_config).await {
            warn!(error = %e, "Failed to initialize message bus; continuing without event publishing");
        } else {
            info!(exchange = %args.message_bus_exchange, "Message bus initialized");
        }
    }

    // Publish startup event
    awful_publish::info!(
        "awful_text_news",
        event_kind = "application.started",
        version = env!("CARGO_PKG_VERSION"),
        "Application starting"
    );

    // Early check: ensure JSON output dir is writable
    if let Err(e) = ensure_writable_dir(&args.json_output_dir).await {
        error!(
            path = %args.json_output_dir,
            error = %e,
            "JSON output directory is not writable (fix perms or choose a different path)"
        );
        awful_publish::error!(
            "awful_text_news",
            event_kind = "application.failed",
            reason = "directory_not_writable",
            path = %args.json_output_dir,
            "Application failed: output directory not writable"
        );
        return Err(e);
    }

    // ---- Index and fetch articles ----
    awful_publish::info!(
        "awful_text_news",
        event_kind = "indexing.started",
        "Starting article indexing from all sources"
    );

    let cnn_urls = scrapers::cnn::index_articles().await?;
    let npr_urls = scrapers::npr::index_articles().await?;
    let apnews_urls = scrapers::apnews::index_articles().await?;
    let aljazeera_urls = scrapers::aljazeera::index_articles().await?;
    let bbcnews_urls = scrapers::bbcnews::index_articles().await?;
    let nyt_articles_with_titles = scrapers::nyt::index_articles(args.nyt_api_key.as_deref()).await?;

    let total_indexed = cnn_urls.len() + npr_urls.len() + apnews_urls.len()
        + aljazeera_urls.len() + bbcnews_urls.len() + nyt_articles_with_titles.len();
    awful_publish::info!(
        "awful_text_news",
        event_kind = "indexing.completed",
        total_urls = total_indexed,
        cnn_count = cnn_urls.len(),
        npr_count = npr_urls.len(),
        apnews_count = apnews_urls.len(),
        aljazeera_count = aljazeera_urls.len(),
        bbcnews_count = bbcnews_urls.len(),
        nyt_count = nyt_articles_with_titles.len(),
        "Article indexing completed"
    );

    awful_publish::info!(
        "awful_text_news",
        event_kind = "fetching.started",
        "Starting article content fetching"
    );

    let cnn_articles = scrapers::cnn::fetch_articles(cnn_urls).await;
    let npr_articles = scrapers::npr::fetch_articles(npr_urls).await;
    let apnews_articles = scrapers::apnews::fetch_articles(apnews_urls).await;
    let aljazeera_articles = scrapers::aljazeera::fetch_articles(aljazeera_urls).await;
    let bbcnews_articles = scrapers::bbcnews::fetch_articles(bbcnews_urls).await;
    let nyt_articles = scrapers::nyt::fetch_articles(nyt_articles_with_titles).await;

    // Capture per-source counts before flattening
    let cnn_fetched = cnn_articles.len();
    let npr_fetched = npr_articles.len();
    let apnews_fetched = apnews_articles.len();
    let aljazeera_fetched = aljazeera_articles.len();
    let bbcnews_fetched = bbcnews_articles.len();
    let nyt_fetched = nyt_articles.len();

    let articles = vec![cnn_articles, npr_articles, apnews_articles, aljazeera_articles, bbcnews_articles, nyt_articles]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    info!(count = articles.len(), "Total articles to analyze");

    awful_publish::info!(
        "awful_text_news",
        event_kind = "fetching.completed",
        total_articles = articles.len(),
        cnn_count = cnn_fetched,
        npr_count = npr_fetched,
        apnews_count = apnews_fetched,
        aljazeera_count = aljazeera_fetched,
        bbcnews_count = bbcnews_fetched,
        nyt_count = nyt_fetched,
        "Article fetching completed"
    );

    // ---- Load template & config ----
    let template = template::load_template("news_parser").await?;
    info!("Loaded template: news_parser");
    let conf_file = config_dir()?.join("config.yaml");
    let config_path = conf_file.to_str().expect("Not a valid config filename");
    let config = config::load_config(config_path).unwrap();
    info!(config_path, "Loaded configuration");
    
    // Wrap config and template in Arc for sharing across parallel tasks
    use std::sync::Arc;
    let config = Arc::new(config);
    let template = Arc::new(template);

    // ---- Build front page ----
    let local_date = Local::now().date_naive().to_string();
    let local_time = Local::now().time().to_string();
    let mut front_page = FrontPage {
        time_of_day: time_of_day(),
        local_time,
        local_date,
        articles: Vec::new(),
    };
    info!(time_of_day = %front_page.time_of_day, local_date = %front_page.local_date, local_time = %front_page.local_time, "FrontPage initialized");

    // ---- Analyze articles in parallel (12 at a time) ----
    use futures::stream::{self, StreamExt};
    const PARALLEL_BATCH_SIZE: usize = 12;

    let total_articles = articles.len();
    info!(parallel_batch_size = PARALLEL_BATCH_SIZE, "Starting parallel article processing");

    awful_publish::info!(
        "awful_text_news",
        event_kind = "processing.started",
        total_articles = total_articles,
        batch_size = PARALLEL_BATCH_SIZE,
        "Starting article processing"
    );
    
    // Process articles concurrently
    let results: Vec<Option<AwfulNewsArticle>> = stream::iter(articles.iter().enumerate())
        .map(|(i, article)| {
            let config = Arc::clone(&config);
            let template = Arc::clone(&template);
            async move {
                debug!(index = i, source = %article.source, "Analyzing article");

                // First ask
                match ask_with_backoff(&config, &article.content, &template).await {
                    Ok(response_json) => {
                        // Try parse
                        let mut parsed = serde_json::from_str::<AwfulNewsArticle>(&response_json);

                        // If the parse failed due to EOF (truncation), re-ask ONCE
                        if let Err(ref e) = parsed {
                            if looks_truncated(e) {
                                warn!(index = i, error = %e, "EOF while parsing; re-asking once");
                                match ask_with_backoff(&config, &article.content, &template).await {
                                    Ok(r2) => {
                                        parsed = serde_json::from_str::<AwfulNewsArticle>(&r2);
                                    }
                                    Err(e2) => {
                                        warn!(index = i, error = %e2, "Re-ask failed; will skip article");
                                    }
                                }
                            }
                        }

                        match parsed {
                            Ok(mut awful_news_article) => {
                                awful_news_article.source = Some(article.source.clone());
                                awful_news_article.content = Some(article.content.clone());

                                // dedupe
                                awful_news_article.namedEntities = awful_news_article
                                    .namedEntities
                                    .into_iter()
                                    .unique_by(|e| e.name.clone())
                                    .collect::<Vec<NamedEntity>>();
                                awful_news_article.importantDates = awful_news_article
                                    .importantDates
                                    .into_iter()
                                    .unique_by(|e| e.descriptionOfWhyDateIsRelevant.clone())
                                    .collect::<Vec<ImportantDate>>();
                                awful_news_article.importantTimeframes = awful_news_article
                                    .importantTimeframes
                                    .into_iter()
                                    .unique_by(|e| e.descriptionOfWhyTimeFrameIsRelevant.clone())
                                    .collect::<Vec<ImportantTimeframe>>();
                                awful_news_article.keyTakeAways = awful_news_article
                                    .keyTakeAways
                                    .into_iter()
                                    .unique()
                                    .collect::<Vec<String>>();

                                info!(index = i, "Successfully processed article");
                                Some(awful_news_article)
                            }
                            Err(e) => {
                                warn!(
                                    index = i,
                                    error = %e,
                                    response_preview = %truncate_for_log(&response_json, 300),
                                    "Model returned non-conforming JSON; skipping article"
                                );
                                None
                            }
                        }
                    }
                    Err(e) => {
                        error!(index = i, source = %article.source, error = %e, "API call failed; skipping article");
                        None
                    }
                }
            }
        })
        .buffer_unordered(PARALLEL_BATCH_SIZE)
        .collect()
        .await;

    // Add successful results to front_page
    for result in results.into_iter().flatten() {
        front_page.articles.push(result);
    }
    
    let successful_count = front_page.articles.len();
    let failed_count = total_articles - successful_count;
    info!(
        total = total_articles,
        successful = successful_count,
        failed = failed_count,
        "Completed parallel article processing"
    );

    awful_publish::info!(
        "awful_text_news",
        event_kind = "processing.completed",
        total_articles = total_articles,
        successful = successful_count,
        failed = failed_count,
        "Article processing completed"
    );

    // Write final JSON after all articles processed
    awful_publish::info!(
        "awful_text_news",
        event_kind = "output.json.started",
        "Writing JSON output"
    );
    if let Err(e) = json::write_frontpage(&front_page, &args.json_output_dir).await {
        error!(error = %e, "Failed to write final JSON");
        awful_publish::error!(
            "awful_text_news",
            event_kind = "output.json.failed",
            "Failed to write JSON output"
        );
    } else {
        awful_publish::info!(
            "awful_text_news",
            event_kind = "output.json.completed",
            article_count = front_page.articles.len(),
            "JSON output written successfully"
        );
    }

    // ---- Markdown output ----
    let md = markdown::front_page_to_markdown(&front_page);
    let output_markdown_filename = format!(
        "{}/{}_{}.md",
        args.markdown_output_dir, front_page.local_date, front_page.time_of_day
    );

    info!(path = %output_markdown_filename, "Writing Markdown");
    awful_publish::info!(
        "awful_text_news",
        event_kind = "output.markdown.started",
        "Writing Markdown output"
    );
    if let Err(e) = tokio::fs::write(&output_markdown_filename, md).await {
        error!(path = %output_markdown_filename, error = %e, "Failed writing Markdown");
        awful_publish::error!(
            "awful_text_news",
            event_kind = "output.markdown.failed",
            path = %output_markdown_filename,
            "Failed to write Markdown output"
        );
    } else {
        info!(path = %output_markdown_filename, "Wrote FrontPage Markdown");
        awful_publish::info!(
            "awful_text_news",
            event_kind = "output.markdown.completed",
            path = %output_markdown_filename,
            "Markdown output written successfully"
        );
    }

    // ---- Index updates ----
    let markdown_filename = format!("{}_{}.md", front_page.local_date, front_page.time_of_day);
    
    if let Err(e) = indexes::update_date_toc_file(
        &args.markdown_output_dir,
        &front_page,
        &markdown_filename,
    )
    .await
    {
        error!(error = %e, "Failed to update date TOC file");
    }

    if let Err(e) = indexes::update_summary_md(
        &args.markdown_output_dir,
        &front_page,
        &markdown_filename,
    )
    .await
    {
        error!(error = %e, "Failed to update SUMMARY.md");
    }

    if let Err(e) = indexes::update_daily_news_index(
        &args.markdown_output_dir,
        &front_page,
        &markdown_filename,
    )
    .await
    {
        error!(error = %e, "Failed to update daily_news.md index");
    }

    let elapsed = start_time.elapsed();
    info!(
        ?elapsed,
        secs = elapsed.as_secs(),
        millis = elapsed.subsec_millis(),
        "Execution complete"
    );

    awful_publish::info!(
        "awful_text_news",
        event_kind = "application.completed",
        duration_secs = elapsed.as_secs(),
        duration_millis = elapsed.subsec_millis(),
        articles_processed = successful_count,
        articles_failed = failed_count,
        edition = %front_page.time_of_day,
        date = %front_page.local_date,
        "Application completed successfully"
    );

    Ok(())
}
