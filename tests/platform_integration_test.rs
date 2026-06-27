#[cfg(test)]
mod platform_tests {
    use igs_rust_mcp::tools::types::*;
    use igs_rust_mcp::tools::types_base::OutputOptions;
    use igs_rust_mcp::tools::youtube;
    use igs_rust_mcp::tools::twitter;

    #[tokio::test]
    async fn test_youtube_search_basic() {
        let input = YoutubeSearchInput {
            query: "rust programming".to_string(),
            limit: Some(3),
        };

        let result = youtube::youtube_search(input).await;
        assert!(result.is_ok(), "YouTube search failed: {:?}", result.err());

        let output = result.unwrap();
        assert!(output.count > 0, "Expected at least 1 result");
        assert!(output.videos.len() > 0, "Videos vector should not be empty");

        let video = &output.videos[0];
        assert!(!video.id.is_empty(), "Video ID should not be empty");
        assert!(!video.title.is_empty(), "Video title should not be empty");
        assert!(video.url.contains("youtube.com"), "URL should contain youtube.com");
        assert!(!video.channel.is_empty(), "Channel should not be empty");

        println!("YouTube search: Found {} videos", output.count);
        println!("  First: {} by {}", video.title, video.channel);
    }

    #[tokio::test]
    async fn test_youtube_metadata() {
        let input = YoutubeMetadataInput {
            url: "https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string(),
        };

        let result = youtube::youtube_metadata(input).await;
        assert!(result.is_ok(), "YouTube metadata failed: {:?}", result.err());

        let output = result.unwrap();
        assert!(!output.title.is_empty(), "Title should not be empty");
        assert!(!output.channel.is_empty(), "Channel should not be empty");
        assert!(output.views.is_some(), "Views should be present");
        assert!(output.duration.is_some(), "Duration should be present");

        println!("YouTube metadata:");
        println!("  Title: {}", output.title);
        println!("  Channel: {}", output.channel);
        println!("  Views: {:?}", output.views);
        println!("  Duration: {:?}", output.duration);
    }

    #[tokio::test]
    async fn test_youtube_subtitles() {
        let input = YoutubeSubtitlesInput {
            url: "https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string(),
            language: Some("en".to_string()),
        };

        let result = youtube::youtube_subtitles(input).await;
        assert!(result.is_ok(), "YouTube subtitles failed: {:?}", result.err());

        let output = result.unwrap();
        assert!(!output.subtitles.is_empty(), "Subtitles should not be empty");
        assert!(!output.language.is_empty(), "Language should not be empty");

        println!("YouTube subtitles: {} chars in {}", output.subtitles.len(), output.language);
        println!("  Preview: {}...", &output.subtitles[..200.min(output.subtitles.len())]);
    }

    #[tokio::test]
    async fn test_youtube_search_empty_query() {
        let input = YoutubeSearchInput {
            query: "".to_string(),
            limit: Some(1),
        };

        let result = youtube::youtube_search(input).await;
        assert!(result.is_err(), "Empty query should return error");
    }

    #[tokio::test]
    async fn test_youtube_metadata_invalid_url() {
        let input = YoutubeMetadataInput {
            url: "https://www.youtube.com/watch?v=INVALID_ID_12345".to_string(),
        };

        let result = youtube::youtube_metadata(input).await;
        assert!(result.is_err(), "Invalid URL should return error");
        println!("Invalid URL error: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_twitter_search_no_cookie() {
        let input = TwitterSearchInput {
            query: "rust programming".to_string(),
            limit: Some(5),
            search_mode: None,
            output: OutputOptions { format: None },
        };

        let result = twitter::twitter_search(input).await;
        assert!(result.is_err(), "Twitter search without cookie should fail");
        let err = result.unwrap_err();
        assert!(err.contains("not configured") || err.contains("disabled"), 
            "Error should mention not configured or disabled: {}", err);
        println!("Twitter no-cookie error: {}", err);
    }

    #[tokio::test]
    async fn test_twitter_read_no_cookie() {
        let input = TwitterReadInput {
            url: "https://x.com/elonmusk/status/1234567890".to_string(),
            output: OutputOptions { format: None },
        };

        let result = twitter::twitter_read(input).await;
        assert!(result.is_err(), "Twitter read without cookie should fail");
        let err = result.unwrap_err();
        assert!(err.contains("not configured") || err.contains("disabled"), 
            "Error should mention not configured or disabled: {}", err);
        println!("Twitter no-cookie error: {}", err);
    }
}
