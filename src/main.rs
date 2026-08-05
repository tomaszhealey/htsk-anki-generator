mod config;

use clap::Parser;
use config::Config;
use fancy_regex::Regex;
use reqwest::header::{HeaderMap, USER_AGENT};
use std::{collections::VecDeque, error::Error, fs, process, usize};
use tokio::runtime::Runtime;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    lesson: usize,
    ///Output path. If set, will take the value of $OUTPUT as the default.
    #[arg(short = 'o', long = "output", default_value_t = "output/".to_string())]
    path: String,
    ///Resets the cache of all urls on "www.howtostudykorean.com". Use if a new lesson is not found.
    #[arg(short = 'r', long = "reset-cache", default_value_t = false)]
    reset: bool,
}

fn main() {
    let config = Config::build(Args::parse()).unwrap_or_else(|err| {
        eprintln!("Problem passing arguments: {err}");
        process::exit(1);
    });

    if let Err(msg) = make_anki(config) {
        eprintln!("{msg}");
        eprintln!("Unable to create anki deck.");
    }
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

struct WordType<'a> {
    name: &'a str,
    next_location: usize,
    markers: VecDeque<(&'a str, usize)>,
}

impl WordType<'_> {
    fn new<'a>(page: &'a str) -> Self {
        let mut markers = VecDeque::new();
        for word_type in ["Verb", "Adjective", "Adverb"] {
            let location = if let Ok(Some(capture)) = Regex::new(word_type).unwrap().find(&page) {
                capture.range().start
            } else {
                usize::MAX
            };
            markers.push_back((word_type, location));
        }

        WordType {
            name: "Noun",
            next_location: markers[0].1,
            markers,
        }
    }

    fn advance(&mut self) {
        self.name = self.markers.pop_front().unwrap().0;
        self.next_location = self.markers.get(0).unwrap_or(&("Empty", usize::MAX)).1;
    }
}

fn make_anki(config: Config) -> Result<(), Box<dyn Error>> {
    println!("Getting lesson...");
    let page = fetch_data(config.url)?;
    let mut word_type = WordType::new(&page);

    //Extracts the vocab from the page and categorises
    println!("Extracting vocab...");
    let mut vocab = Vec::new();
    let re = Regex::new(".mp3\">(\\N+)<\\/\\N+>(\\N+)<\\/span>").unwrap();
    for capture in re.captures_iter(&page).map(|c| c.unwrap()) {
        if word_type.next_location < capture.get(0).unwrap().range().start {
            word_type.advance();
        }
        vocab.push((
            word_type.name,
            capture.get(1).unwrap().as_str(),
            capture.get(2).unwrap().as_str(),
        ));
    }

    println!("Making deck...");
    let mut deck = String::from(
        "#separator:tab
#html:true
#deck column:3
#tags column:4\n\n",
    );

    let unit = config.unit;
    for (category, kr, en) in vocab {
        deck.push_str(&format!(
            "{en}\t{kr}\tHTSK::Unit {unit}::{category}s\thtsk::unit-{unit} {}\n",
            category.to_lowercase()
        ));
    }

    if !fs::exists(&config.output_dir).unwrap() {
        fs::create_dir(&config.output_dir)?;
    }

    let dir = config
        .output_dir
        .join(format!("Unit-{unit}-Lesson-{}", config.lesson) + ".txt");
    std::fs::write(&dir, deck)?;
    println!("Created deck at \"{}\"", dir.to_str().unwrap());

    Ok(())
}
