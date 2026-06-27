use scraper::{Html, Selector};

#[tokio::test]
async fn debug_reddit_html() {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    
    let resp = client
        .get("https://old.reddit.com/r/rust/search?q=rust+programming&restrict_sr=on&sort=relevance&t=all&limit=5")
        .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36")
        .send()
        .await
        .unwrap();
    
    println!("Status: {}", resp.status());
    println!("Headers: {:#?}", resp.headers());
    
    let body = resp.text().await.unwrap();
    println!("Body length: {}", body.len());
    println!("First 500 chars: {}", &body[..500.min(body.len())]);
    
    println!("\n--- Testing with redirect policy = none ---");
    
    let search_count = body.matches("search-result-link").count();
    println!("'search-result-link' appears {} times", search_count);
    
    let selectors_to_try = vec![
        "div.search-result.search-result-link",
        "[class*='search-result']",
        "faceplate-tracker[n='post']",
        "[data-testid='post-container']",
        "a[click-id='search_result']",
    ];
    
    let document = Html::parse_document(&body);
    for sel_str in selectors_to_try {
        if let Ok(sel) = Selector::parse(sel_str) {
            let count = document.select(&sel).count();
            println!("Selector '{}' found {} elements", sel_str, count);
        }
    }
}
