mod config;

use config::Config;
use reqwest::header::{HeaderMap, USER_AGENT};
use std::{env, process};
use tokio::runtime::Runtime;

fn main() {
    let config = Config::build(env::args()).unwrap_or_else(|err| {
        eprintln!("Problem passing arguments: {err}");
        process::exit(1);
    });

    println!(
        "Unit: {}, Lesson: {}, URL: {}",
        config.unit, config.lesson, config.url
    );
}

pub fn fetch_data(url: String) -> Result<String, reqwest::Error> {
    Runtime::new().unwrap().handle().block_on(async {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, "rust-web-scraper/0.1".parse().unwrap());

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await
    })
}
