//! JSON output generation for the API.
//!
//! This module serializes processed articles to JSON format for consumption
//! by external clients and APIs.
//!
//! # Output Structure
//!
//! Files are organized by date with edition names:
//! ```text
//! json_output_dir/
//! └── 2025-05-06/
//!     ├── morning.json
//!     ├── afternoon.json
//!     └── evening.json
//! ```
//!
//! # Evening Edge Case
//!
//! If an "evening" edition runs just after midnight (before the date changes),
//! it uses yesterday's date to keep the edition logically grouped with the
//! correct day's news.

use crate::models::FrontPage;
use chrono::{Duration, Local, NaiveTime};
use std::error::Error;
use tokio::fs;
use tracing::{error, info, instrument};

/// Write a [`FrontPage`] to a JSON file with date-based directory structure.
///
/// Creates the necessary directory structure and writes the serialized
/// `FrontPage` as JSON. The file path is determined by the date and
/// time-of-day from the `FrontPage` data.
///
/// # Arguments
///
/// * `front_page` - The processed articles to serialize
/// * `json_output_dir` - Base directory for JSON output
///
/// # Returns
///
/// `Ok(())` on success, or an error if directory creation or file writing fails.
///
/// # Output Path
///
/// The file is written to: `{json_output_dir}/{date}/{time_of_day}.json`
#[instrument(level = "info", skip_all, fields(json_output_dir = %json_output_dir))]
pub async fn write_frontpage(
    front_page: &FrontPage,
    json_output_dir: &str,
) -> Result<(), Box<dyn Error>> {
    let json = serde_json::to_string(front_page)?;

    let midnight = NaiveTime::from_hms_opt(23, 59, 59).unwrap();
    let now = Local::now().time();
    let yesterday = Local::now().date_naive() - Duration::days(1);

    let full_json_dir = if front_page.time_of_day == "evening" && (now >= midnight) {
        format!("{}/{}", json_output_dir, yesterday.to_string())
    } else {
        format!("{}/{}", json_output_dir, front_page.local_date)
    };

    info!(%full_json_dir, "Ensuring JSON directory exists");
    if let Err(e) = fs::create_dir_all(&full_json_dir).await {
        error!(%full_json_dir, error = %e, "Failed to create JSON dir");
        return Err(e.into());
    }

    let output_json_filename = if front_page.time_of_day == "evening" && (now >= midnight) {
        format!("{}/{}.json", full_json_dir, yesterday.to_string())
    } else {
        format!("{}/{}.json", full_json_dir, front_page.time_of_day)
    };

    info!(path = %output_json_filename, "Writing JSON");
    fs::write(&output_json_filename, json).await?;
    info!(path = %output_json_filename, "Wrote JSON API file");

    Ok(())
}
