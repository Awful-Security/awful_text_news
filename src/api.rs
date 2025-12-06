//! LLM API interaction with exponential backoff retry logic.
//!
//! This module provides a robust interface for communicating with an
//! OpenAI-compatible LLM API. It includes automatic retry logic with
//! exponential backoff and jitter to handle transient failures gracefully.
//!
//! # Architecture
//!
//! The module uses a trait-based design for flexibility:
//! - [`AskAsync`]: Core trait defining async LLM interaction
//! - [`AskFnWrapper`]: Wraps the `awful_aj` library's `ask` function
//! - [`RetryAsk`]: Decorator that adds retry logic to any `AskAsync` implementation
//!
//! # Retry Strategy
//!
//! - Maximum 5 retry attempts
//! - Exponential backoff starting at 1 second
//! - Maximum delay capped at 30 seconds
//! - Random jitter (0-250ms) added to prevent thundering herd

use awful_aj::api::ask;
use awful_aj::{config::AwfulJadeConfig, template::ChatTemplate};
use rand::{rng, Rng};
use std::error::Error;
use std::fmt;
use std::time::{Duration as StdDuration, Instant};
use tokio::time::sleep;
use tracing::{error, info, instrument, warn};

/// Trait for async LLM interaction.
///
/// Implementors of this trait can send text to an LLM and receive a response.
/// This abstraction allows for different LLM backends or decorators (like retry logic).
pub trait AskAsync {
    /// The type of response returned by the LLM.
    type Response;

    /// Send text to the LLM and receive a response.
    ///
    /// # Arguments
    ///
    /// * `text` - The input text to send to the LLM
    ///
    /// # Returns
    ///
    /// The LLM's response, or an error if the request failed.
    async fn ask(&self, text: &str) -> Result<Self::Response, Box<dyn Error>>;
}

/// Wrapper that adds exponential backoff retry logic to any [`AskAsync`] implementation.
///
/// This decorator transparently adds retry logic with exponential backoff
/// and jitter to handle transient API failures. It's designed to be resilient
/// against rate limiting, network issues, and temporary server errors.
///
/// # Backoff Strategy
///
/// The delay between retries follows this formula:
/// ```text
/// delay = min(base_delay * 2^(attempt-1), max_delay) + random_jitter(0..250ms)
/// ```
pub struct RetryAsk<T> {
    /// The underlying LLM client to wrap.
    inner: T,
    /// Maximum number of retry attempts before giving up.
    max_retries: usize,
    /// Initial delay between retries (doubles with each attempt).
    base_delay: StdDuration,
    /// Maximum delay cap to prevent excessive waiting.
    max_delay: StdDuration,
}

impl<T> RetryAsk<T>
where
    T: AskAsync,
{
    /// Create a new retry wrapper around an existing [`AskAsync`] implementation.
    ///
    /// # Arguments
    ///
    /// * `inner` - The underlying LLM client to wrap
    /// * `max_retries` - Maximum number of retry attempts (5 recommended)
    /// * `base_delay` - Initial delay between retries (1 second recommended)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let client = AskFnWrapper { config, template };
    /// let retry_client = RetryAsk::new(client, 5, Duration::from_secs(1));
    /// ```
    pub fn new(inner: T, max_retries: usize, base_delay: StdDuration) -> Self {
        Self {
            inner,
            max_retries,
            base_delay,
            max_delay: StdDuration::from_secs(30),
        }
    }
}

impl<T> fmt::Debug for RetryAsk<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RetryAsk")
            .field("max_retries", &self.max_retries)
            .field("base_delay", &self.base_delay)
            .field("max_delay", &self.max_delay)
            .finish()
    }
}

impl<T> AskAsync for RetryAsk<T>
where
    T: AskAsync + fmt::Debug,
{
    type Response = T::Response;

    #[instrument(level = "info", skip_all)]
    async fn ask(&self, text: &str) -> Result<Self::Response, Box<dyn Error>> {
        let total_t0 = Instant::now();
        let mut attempt = 0usize;

        loop {
            let attempt_t0 = Instant::now();
            match self.inner.ask(text).await {
                Ok(resp) => {
                    return Ok(resp);
                }
                Err(e) => {
                    attempt += 1;
                    let attempt_dt = attempt_t0.elapsed();
                    let total_dt = total_t0.elapsed();

                    if attempt > self.max_retries {
                        error!(
                            attempt,
                            max = self.max_retries,
                            elapsed_ms_attempt = attempt_dt.as_millis() as u128,
                            elapsed_ms_total = total_dt.as_millis() as u128,
                            error = %e,
                            "ask() exhausted retries"
                        );
                        return Err(e);
                    }

                    // backoff calc
                    let mut delay = self.base_delay.saturating_mul(1 << (attempt - 1));
                    if delay > self.max_delay {
                        delay = self.max_delay;
                    }
                    let jitter_ms: u64 = rng().random_range(0..=250);
                    let delay = delay + StdDuration::from_millis(jitter_ms);

                    warn!(
                        attempt,
                        max = self.max_retries,
                        elapsed_ms_attempt = attempt_dt.as_millis() as u128,
                        elapsed_ms_total = total_dt.as_millis() as u128,
                        ?delay,
                        error = %e,
                        "ask() attempt failed; backing off"
                    );
                    sleep(delay).await;
                }
            }
        }
    }
}

/// Wrapper around `awful_aj::api::ask` that implements [`AskAsync`].
///
/// This struct adapts the `awful_aj` library's `ask` function to work with
/// the [`AskAsync`] trait, enabling it to be used with [`RetryAsk`] and
/// other decorators.
///
/// # Lifetime Parameters
///
/// * `'a` - The lifetime of the references to config and template
#[derive(Debug)]
pub struct AskFnWrapper<'a> {
    /// Reference to the LLM configuration (API keys, endpoints, model settings).
    pub config: &'a AwfulJadeConfig,
    /// Reference to the chat template defining the conversation structure.
    pub template: &'a ChatTemplate,
}

impl<'a> AskAsync for AskFnWrapper<'a> {
    type Response = String;

    #[instrument(level = "info", skip_all)]
    async fn ask(&self, text: &str) -> Result<Self::Response, Box<dyn Error>> {
        let t0 = Instant::now();
        let res = ask(self.config, text.to_string(), self.template, None, None).await;
        let dt = t0.elapsed();

        match &res {
            Ok(_) => {}
            Err(e) => warn!(elapsed_ms = dt.as_millis() as u128, error = %e, "API call failed"),
        }
        res
    }
}

/// High-level function to call LLM with exponential backoff retry logic.
///
/// This is the primary entry point for sending article content to the LLM.
/// It automatically wraps the request with retry logic to handle transient
/// failures gracefully.
///
/// # Arguments
///
/// * `config` - LLM configuration (API endpoint, model, etc.)
/// * `article` - The article text to process
/// * `template` - The chat template defining the conversation structure
///
/// # Returns
///
/// The LLM's response as a JSON string containing the processed article data,
/// or an error if all retry attempts fail.
///
/// # Retry Behavior
///
/// - Up to 5 retry attempts
/// - Exponential backoff: 1s, 2s, 4s, 8s, 16s (capped at 30s)
/// - Random jitter added to prevent thundering herd
#[instrument(level = "info", skip_all)]
pub async fn ask_with_backoff(
    config: &AwfulJadeConfig,
    article: &String,
    template: &ChatTemplate,
) -> Result<String, Box<dyn Error>> {
    let t0 = Instant::now();
    let client = AskFnWrapper { config, template };
    let api = RetryAsk::new(client, 5, StdDuration::from_secs(1));
    let res = api.ask(article).await;
    let dt = t0.elapsed();

    match &res {
        Ok(_) => info!(
            elapsed_ms_total = dt.as_millis() as u128,
            "ask_with_backoff succeeded"
        ),
        Err(e) => {
            error!(elapsed_ms_total = dt.as_millis() as u128, error = %e, "ask_with_backoff failed")
        }
    }
    res
}
