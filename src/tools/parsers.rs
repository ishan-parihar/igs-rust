use crate::tools::types::*;

/// List available parser keys
pub async fn parsers_list() -> Result<ParserListOutput, String> {
    Ok(ParserListOutput {
        parsers: vec![
            ParserInfo { key: "rss".into(), note: "Generic RSS/Atom via feed-rs".into() },
            ParserInfo { key: "ofac".into(), note: "OFAC Recent Actions HTML parser".into() },
            ParserInfo { key: "ussf_cfc".into(), note: "US Space Force CFC News HTML parser".into() },
            ParserInfo { key: "who_dons".into(), note: "WHO Disease Outbreak News JSON parser".into() },
            ParserInfo { key: "newslaundry".into(), note: "Newslaundry list page JSON-in-script parser".into() },
            ParserInfo { key: "generic_html".into(), note: "Generic HTML scraper with auto-detect".into() },
            ParserInfo { key: "hackernews".into(), note: "Hacker News Algolia JSON API".into() },
            ParserInfo { key: "youtube".into(), note: "YouTube channel RSS/Atom feed".into() },
            ParserInfo { key: "github".into(), note: "GitHub releases + search JSON API".into() },
            ParserInfo { key: "bluesky".into(), note: "Bluesky AT Protocol JSON API".into() },
            ParserInfo { key: "semantic_scholar".into(), note: "Semantic Scholar JSON API".into() },
        ],
    })
}
