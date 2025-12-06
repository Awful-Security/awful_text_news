//! Data models for news articles and their processed representations.
//!
//! This module defines the core data structures used throughout the application:
//! - [`NewsArticle`]: Raw scraped article data from news sources
//! - [`FrontPage`]: Collection of processed articles for a single edition
//! - [`AwfulNewsArticle`]: LLM-processed article with extracted metadata
//! - Entity types: [`NamedEntity`], [`ImportantDate`], [`ImportantTimeframe`]
//!
//! The models use camelCase field names to match the JSON schema expected by
//! the LLM, hence the `#[allow(non_snake_case)]` attributes.

use serde::{Deserialize, Serialize};

/// A raw news article as scraped from a news source.
///
/// This struct represents the unprocessed article content before it is
/// sent to the LLM for summarization and entity extraction.
///
/// # Fields
///
/// * `source` - The URL where the article was scraped from
/// * `content` - The raw text content of the article
#[derive(Debug)]
pub struct NewsArticle {
    /// The source URL of the article.
    pub source: String,
    /// The raw text content scraped from the article.
    pub content: String,
}

/// A collection of processed articles representing a single news edition.
///
/// Each execution of the application produces one `FrontPage`, which is
/// serialized to both JSON (for API consumption) and Markdown (for reading).
///
/// # Edition Naming
///
/// The `time_of_day` field categorizes editions as:
/// - `"morning"`: 00:00 - 08:00
/// - `"afternoon"`: 08:00 - 16:00
/// - `"evening"`: 16:00 - 24:00
#[derive(Debug, Deserialize, Serialize)]
pub struct FrontPage {
    /// The date of publication in `YYYY-MM-DD` format.
    pub local_date: String,
    /// The time of day category: "morning", "afternoon", or "evening".
    pub time_of_day: String,
    /// The exact local time of publication in `HH:MM:SS.microseconds` format.
    pub local_time: String,
    /// The collection of processed articles in this edition.
    pub articles: Vec<AwfulNewsArticle>,
}

/// A fully processed news article with LLM-extracted metadata.
///
/// This struct represents an article after it has been processed by the LLM.
/// It contains the original source information along with extracted summaries,
/// entities, dates, and other structured data.
///
/// # JSON Schema
///
/// The field names use camelCase to match the JSON schema defined in the
/// LLM template. This ensures consistent serialization/deserialization
/// when communicating with the LLM API.
#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct AwfulNewsArticle {
    /// The original source URL (added after LLM processing).
    pub source: Option<String>,
    /// The article's publication date as extracted by the LLM.
    pub dateOfPublication: String,
    /// The article's publication time as extracted by the LLM.
    pub timeOfPublication: String,
    /// The article title/headline.
    pub title: String,
    /// The category assigned by the LLM (e.g., "Politics & Governance", "Science & Technology").
    pub category: String,
    /// A concise summary of the article content.
    pub summaryOfNewsArticle: String,
    /// Key points or takeaways from the article.
    pub keyTakeAways: Vec<String>,
    /// People, organizations, and other entities mentioned in the article.
    pub namedEntities: Vec<NamedEntity>,
    /// Significant dates mentioned in the article.
    pub importantDates: Vec<ImportantDate>,
    /// Significant time periods or ranges mentioned in the article.
    pub importantTimeframes: Vec<ImportantTimeframe>,
    /// Topic tags assigned by the LLM.
    pub tags: Vec<String>,
    /// The original article content (added after LLM processing).
    pub content: Option<String>,
}

impl AwfulNewsArticle {
    /// Extract the domain name (before .com/.org/etc) from the source URL
    /// For example: "https://lite.cnn.com/article" -> "cnn"
    pub fn source_tag(&self) -> Option<String> {
        self.source.as_ref().and_then(|url| {
            // Parse the URL and extract the host
            if let Ok(parsed) = url::Url::parse(url) {
                if let Some(host) = parsed.host_str() {
                    // Split by dots and get the domain before the TLD
                    let parts: Vec<&str> = host.split('.').collect();
                    // Handle cases like "lite.cnn.com" -> "cnn" or "cnn.com" -> "cnn"
                    if parts.len() >= 2 {
                        // Get the second-to-last part (domain before TLD)
                        return Some(parts[parts.len() - 2].to_string());
                    }
                }
            }
            None
        })
    }
}

/// A named entity (person, organization, place, etc.) extracted from an article.
///
/// Named entities help readers quickly identify key players and organizations
/// mentioned in a news story without reading the full article.
///
/// # Examples
///
/// - Person: "Joe Biden" - "President of the United States"
/// - Organization: "NATO" - "Military alliance"
/// - Place: "Kyiv" - "Capital city of Ukraine"
#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct NamedEntity {
    /// The name of the entity.
    pub name: String,
    /// A brief description of what this entity is.
    pub whatIsThisEntity: String,
    /// Explanation of why this entity is relevant to the article.
    pub whyIsThisEntityRelevantToTheArticle: String,
}

/// A significant date mentioned in an article.
///
/// Important dates help readers understand the timeline of events
/// and when key moments occurred or are scheduled to occur.
#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct ImportantDate {
    /// The date as mentioned in the article (may be in various formats).
    pub dateMentionedInArticle: String,
    /// Explanation of why this date is significant to the story.
    pub descriptionOfWhyDateIsRelevant: String,
}

/// A significant time period or range mentioned in an article.
///
/// Important timeframes help readers understand durations and periods
/// of time that are relevant to the story, such as policy windows,
/// event durations, or historical periods.
#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct ImportantTimeframe {
    /// The start of the time period.
    pub approximateTimeFrameStart: String,
    /// The end of the time period.
    pub approximateTimeFrameEnd: String,
    /// Explanation of why this timeframe is significant to the story.
    pub descriptionOfWhyTimeFrameIsRelevant: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_news_article_creation() {
        let article = NewsArticle {
            source: "https://example.com".to_string(),
            content: "Test content".to_string(),
        };
        assert_eq!(article.source, "https://example.com");
        assert_eq!(article.content, "Test content");
    }

    #[test]
    fn test_frontpage_serialization() {
        let frontpage = FrontPage {
            local_date: "2025-05-06".to_string(),
            time_of_day: "evening".to_string(),
            local_time: "20:30:00".to_string(),
            articles: vec![],
        };

        let json = serde_json::to_string(&frontpage).unwrap();
        assert!(json.contains("2025-05-06"));
        assert!(json.contains("evening"));
    }

    #[test]
    fn test_frontpage_deserialization() {
        let json = r#"{
            "local_date": "2025-05-06",
            "time_of_day": "morning",
            "local_time": "08:00:00",
            "articles": []
        }"#;

        let frontpage: FrontPage = serde_json::from_str(json).unwrap();
        assert_eq!(frontpage.local_date, "2025-05-06");
        assert_eq!(frontpage.time_of_day, "morning");
        assert_eq!(frontpage.articles.len(), 0);
    }

    #[test]
    fn test_awful_news_article_with_entities() {
        let article = AwfulNewsArticle {
            source: Some("https://example.com".to_string()),
            dateOfPublication: "2025-05-06".to_string(),
            timeOfPublication: "14:30:00".to_string(),
            title: "Test Article".to_string(),
            category: "Politics & Governance".to_string(),
            summaryOfNewsArticle: "Summary here".to_string(),
            keyTakeAways: vec!["Key point 1".to_string()],
            namedEntities: vec![NamedEntity {
                name: "Entity Name".to_string(),
                whatIsThisEntity: "Description".to_string(),
                whyIsThisEntityRelevantToTheArticle: "Relevance".to_string(),
            }],
            importantDates: vec![],
            importantTimeframes: vec![],
            tags: vec!["politics".to_string(), "news".to_string()],
            content: Some("Full content".to_string()),
        };

        assert_eq!(article.title, "Test Article");
        assert_eq!(article.category, "Politics & Governance");
        assert_eq!(article.tags.len(), 2);
        assert_eq!(article.namedEntities.len(), 1);
        assert_eq!(article.namedEntities[0].name, "Entity Name");
    }

    #[test]
    fn test_named_entity_serialization() {
        let entity = NamedEntity {
            name: "John Doe".to_string(),
            whatIsThisEntity: "A person".to_string(),
            whyIsThisEntityRelevantToTheArticle: "Main subject".to_string(),
        };

        let json = serde_json::to_string(&entity).unwrap();
        let deserialized: NamedEntity = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "John Doe");
    }

    #[test]
    fn test_important_date_structure() {
        let date = ImportantDate {
            dateMentionedInArticle: "2025-12-25".to_string(),
            descriptionOfWhyDateIsRelevant: "Christmas Day".to_string(),
        };

        assert_eq!(date.dateMentionedInArticle, "2025-12-25");
    }

    #[test]
    fn test_important_timeframe_structure() {
        let timeframe = ImportantTimeframe {
            approximateTimeFrameStart: "2025-01-01".to_string(),
            approximateTimeFrameEnd: "2025-12-31".to_string(),
            descriptionOfWhyTimeFrameIsRelevant: "Full year 2025".to_string(),
        };

        assert_eq!(timeframe.approximateTimeFrameStart, "2025-01-01");
        assert_eq!(timeframe.approximateTimeFrameEnd, "2025-12-31");
    }

    #[test]
    fn test_source_tag_cnn() {
        let article = AwfulNewsArticle {
            source: Some("https://lite.cnn.com/2025/05/06/article".to_string()),
            dateOfPublication: "2025-05-06".to_string(),
            timeOfPublication: "14:30:00".to_string(),
            title: "Test".to_string(),
            category: "Politics & Governance".to_string(),
            summaryOfNewsArticle: "Summary".to_string(),
            keyTakeAways: vec![],
            namedEntities: vec![],
            importantDates: vec![],
            importantTimeframes: vec![],
            tags: vec![],
            content: None,
        };

        assert_eq!(article.source_tag(), Some("cnn".to_string()));
    }

    #[test]
    fn test_source_tag_npr() {
        let article = AwfulNewsArticle {
            source: Some("https://text.npr.org/article".to_string()),
            dateOfPublication: "2025-05-06".to_string(),
            timeOfPublication: "14:30:00".to_string(),
            title: "Test".to_string(),
            category: "Politics & Governance".to_string(),
            summaryOfNewsArticle: "Summary".to_string(),
            keyTakeAways: vec![],
            namedEntities: vec![],
            importantDates: vec![],
            importantTimeframes: vec![],
            tags: vec![],
            content: None,
        };

        assert_eq!(article.source_tag(), Some("npr".to_string()));
    }

    #[test]
    fn test_source_tag_no_source() {
        let article = AwfulNewsArticle {
            source: None,
            dateOfPublication: "2025-05-06".to_string(),
            timeOfPublication: "14:30:00".to_string(),
            title: "Test".to_string(),
            category: "Politics & Governance".to_string(),
            summaryOfNewsArticle: "Summary".to_string(),
            keyTakeAways: vec![],
            namedEntities: vec![],
            importantDates: vec![],
            importantTimeframes: vec![],
            tags: vec![],
            content: None,
        };

        assert_eq!(article.source_tag(), None);
    }

    #[test]
    fn test_source_tag_simple_domain() {
        let article = AwfulNewsArticle {
            source: Some("https://example.com/article".to_string()),
            dateOfPublication: "2025-05-06".to_string(),
            timeOfPublication: "14:30:00".to_string(),
            title: "Test".to_string(),
            category: "Politics & Governance".to_string(),
            summaryOfNewsArticle: "Summary".to_string(),
            keyTakeAways: vec![],
            namedEntities: vec![],
            importantDates: vec![],
            importantTimeframes: vec![],
            tags: vec![],
            content: None,
        };

        assert_eq!(article.source_tag(), Some("example".to_string()));
    }
}
