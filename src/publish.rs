//! Event publishing abstraction with feature-gated implementation.
//!
//! This module provides a unified interface for publishing events to a message bus.
//! When the `publish` feature is enabled, events are sent to RabbitMQ via the
//! `awful_publish` crate. When disabled, all functions and macros are no-ops,
//! allowing the main code to call them unconditionally without `#[cfg]` directives
//! scattered throughout the codebase.
//!
//! # Design Pattern
//!
//! This module uses "duck typing" via macros to provide a consistent API regardless
//! of whether the feature is enabled. The [`publish_info!`] and [`publish_error!`]
//! macros expand to either real publishing calls or empty blocks depending on the
//! feature flag.
//!
//! # Non-Intrusive Design
//!
//! This module uses `awful_publish::init_global()` for initialization and
//! `awful_publish::publish()` for sending events. The `publish()` function
//! sends events directly to RabbitMQ without going through the tracing subscriber,
//! ensuring no interference with the application's existing logging setup.
//!
//! # Events Published
//!
//! When enabled, the application publishes the following events:
//!
//! | Event Kind | Description |
//! |------------|-------------|
//! | `application.started` | Application startup with version info |
//! | `application.failed` | Fatal error preventing execution |
//! | `application.completed` | Successful completion with statistics |
//! | `indexing.started` | Beginning URL discovery from sources |
//! | `indexing.completed` | URL discovery finished with counts |
//! | `fetching.started` | Beginning article content download |
//! | `fetching.completed` | Content download finished with per-source counts |
//! | `processing.started` | Beginning LLM processing |
//! | `processing.completed` | LLM processing finished with success/failure counts |
//! | `output.json.started` | Beginning JSON file write |
//! | `output.json.completed` | JSON file written successfully |
//! | `output.json.failed` | JSON file write failed |
//! | `output.markdown.started` | Beginning Markdown file write |
//! | `output.markdown.completed` | Markdown file written successfully |
//! | `output.markdown.failed` | Markdown file write failed |
//!
//! # Usage
//!
//! ```ignore
//! use crate::publish;
//!
//! // Initialize the message bus (no-op if feature disabled)
//! publish::init(Some(&"amqp://localhost:5672".to_string()), "events").await;
//!
//! // Publish events using macros (no-op if feature disabled)
//! publish_info!(
//!     "awful_text_news",
//!     event_kind = "application.started",
//!     version = "1.0.0",
//!     "Application starting"
//! );
//!
//! publish_error!(
//!     "awful_text_news",
//!     event_kind = "application.failed",
//!     reason = "config_error",
//!     "Failed to load configuration"
//! );
//! ```
//!
//! # Feature Flag
//!
//! Enable with: `cargo build --features publish`
//!
//! Requires access to the private `awful_publish` repository.

/// Initialize the message bus connection.
///
/// Connects to an AMQP broker (e.g., RabbitMQ) and starts the background
/// publisher task.
///
/// # Arguments
///
/// * `amqp_url` - Optional AMQP connection URL (e.g., `amqp://localhost:5672`)
/// * `exchange` - The exchange name to publish events to
///
/// # Returns
///
/// * `true` if the connection was established successfully
/// * `false` if no URL was provided or connection failed
///
/// # Behavior
///
/// * **Feature enabled**: Attempts to connect; logs warning on failure but
///   allows the application to continue without event publishing
/// * **Feature disabled**: Always returns `false` (no-op)
#[cfg(feature = "publish")]
pub async fn init(amqp_url: Option<&String>, exchange: &str) -> bool {
    use awful_publish::BusConfig;
    use tracing::{info, warn};

    if let Some(url) = amqp_url {
        let config = BusConfig::new(url.clone(), exchange.to_string());
        if let Err(e) = awful_publish::init_global(config).await {
            warn!(error = %e, "Failed to initialize message bus; continuing without event publishing");
            false
        } else {
            info!(exchange = %exchange, "Message bus initialized");
            true
        }
    } else {
        false
    }
}

/// Initialize the message bus connection (no-op when `publish` feature is disabled).
#[cfg(not(feature = "publish"))]
pub async fn init(_amqp_url: Option<&String>, _exchange: &str) -> bool {
    false
}

/// Publish an info-level event to the message bus.
///
/// This macro calls `awful_publish::publish()` directly when the `publish` feature
/// is enabled. When disabled, it expands to an empty block.
///
/// # Syntax
///
/// Uses tracing-style syntax with `field = value` pairs followed by a message literal:
///
/// ```ignore
/// publish_info!(service, field1 = value1, field2 = value2, "message");
/// ```
///
/// # Arguments
///
/// * `$service` - The service identifier (e.g., `"awful_text_news"`)
/// * `$key = $value` - Key-value pairs for event fields (supports dotted keys like `foo.bar`)
/// * `$msg` - The event message (must be a string literal)
///
/// # Example
///
/// ```ignore
/// publish_info!(
///     "awful_text_news",
///     event_kind = "indexing.completed",
///     total_urls = 150,
///     "Article indexing completed"
/// );
/// ```
#[cfg(feature = "publish")]
#[macro_export]
macro_rules! publish_info {
    ($service:expr, $($($k:ident).+ = $val:expr),+ , $msg:literal) => {
        awful_publish::publish(
            $service,
            tracing::Level::INFO,
            $msg,
            vec![$(
                (stringify!($($k).+), serde_json::json!($val)),
            )+],
        )
    };
    ($service:expr, $msg:literal) => {
        awful_publish::publish(
            $service,
            tracing::Level::INFO,
            $msg,
            vec![],
        )
    };
}

/// Publish an info-level event (no-op when `publish` feature is disabled).
#[cfg(not(feature = "publish"))]
#[macro_export]
macro_rules! publish_info {
    ($service:expr, $($tt:tt)*) => {};
}

/// Publish an error-level event to the message bus.
///
/// This macro calls `awful_publish::publish()` directly when the `publish` feature
/// is enabled. When disabled, it expands to an empty block.
///
/// # Syntax
///
/// Uses tracing-style syntax with `field = value` pairs followed by a message literal:
///
/// ```ignore
/// publish_error!(service, field1 = value1, field2 = value2, "message");
/// ```
///
/// # Arguments
///
/// * `$service` - The service identifier (e.g., `"awful_text_news"`)
/// * `$key = $value` - Key-value pairs for event fields (supports dotted keys like `foo.bar`)
/// * `$msg` - The event message (must be a string literal)
///
/// # Example
///
/// ```ignore
/// publish_error!(
///     "awful_text_news",
///     event_kind = "output.json.failed",
///     path = "/tmp/output.json",
///     "Failed to write JSON output"
/// );
/// ```
#[cfg(feature = "publish")]
#[macro_export]
macro_rules! publish_error {
    ($service:expr, $($($k:ident).+ = $val:expr),+ , $msg:literal) => {
        awful_publish::publish(
            $service,
            tracing::Level::ERROR,
            $msg,
            vec![$(
                (stringify!($($k).+), serde_json::json!($val)),
            )+],
        )
    };
    ($service:expr, $msg:literal) => {
        awful_publish::publish(
            $service,
            tracing::Level::ERROR,
            $msg,
            vec![],
        )
    };
}

/// Publish an error-level event (no-op when `publish` feature is disabled).
#[cfg(not(feature = "publish"))]
#[macro_export]
macro_rules! publish_error {
    ($service:expr, $($tt:tt)*) => {};
}

// Re-export macros at module level
#[allow(unused_imports)]
pub use publish_error;
#[allow(unused_imports)]
pub use publish_info;
