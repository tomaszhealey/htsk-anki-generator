mod config;

use clap::Parser;
use config::Config;
use fancy_regex::{Captures, Regex};
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
    #[arg(short = 'd', long = "debug", default_value_t = false)]
    debug: bool,
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

pub fn fetch_data(url: &str) -> Result<String, reqwest::Error> {
    Runtime::new().unwrap().handle().block_on(async {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, "rust-web-scraper/0.1".parse().unwrap());

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        client
            .get(url)
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
    fn new<'a>(page: &'a str, lesson: usize) -> Self {
        let mut markers: VecDeque<(&str, usize)> = VecDeque::new();

        let queries = TypeQuery::make_queries(lesson);

        'outer: for query in queries {
            let location = if let Ok(Some(capture)) = Regex::new(query.query).unwrap().find(&page) {
                capture.range().start
            } else {
                usize::MAX
            };

            //So `markers` is in ascending order
            for (index, item) in markers.iter().enumerate() {
                if item.1 > location {
                    markers.insert(index, (query.id, location));
                    continue 'outer;
                }
            }
            markers.push_back((query.id, location));
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

struct TypeQuery<'a> {
    id: &'a str,
    query: &'a str,
}

impl TypeQuery<'_> {
    fn make_queries<'a>(lesson: usize) -> Vec<TypeQuery<'a>> {
        let mut queries = Vec::new();
        queries.push(TypeQuery::new(
            "Verb",
            match lesson {
                5 => "<p>Verbs:",
                7 | 11 | 12 => "Verbs",
                _ => "Verbs:",
            },
        ));
        queries.push(TypeQuery::new(
            "Adjective",
            match lesson {
                7 | 11 | 12 => "Adjectives",
                _ => "Adjectives:",
            },
        ));
        queries.push(TypeQuery::new(
            "Other",
            match lesson {
                8 => "Adverbs:",
                _ => "Adverbs",
            },
        ));

        queries
    }

    fn new<'a>(id: &'a str, query: &'a str) -> TypeQuery<'a> {
        TypeQuery { id, query }
    }
}

fn add_vocab<'a>(
    vocab: &mut Vec<(&'a str, &'a str, &'a str)>,
    capture: Captures<'a, String>,
    word_type: &'a str,
    debug: bool,
) {
    let captures = (1..3)
        .map(|i| capture.get(i).unwrap().as_str())
        .collect::<Vec<_>>();

    if captures[0].len() < 20 && captures[1].len() < 40 {
        vocab.push((word_type, captures[0], captures[1]));
    } else if debug {
        println!("Rejected {captures:?}");
    }
}

fn make_anki(config: Config) -> Result<(), Box<dyn Error>> {
    println!("Getting lesson...");
    let page = fetch_data(&config.url)?;
    let mut word_type = WordType::new(&page, config.lesson);

    if config.debug {
        println!("{page}");
    }

    //Extracts the vocab from the page and categorises
    println!("Extracting vocab...");
    let mut vocab = Vec::new();
    let re = Regex::new("mp3\">(\\N+)<\\/\\N+>(\\N+)<\\/span>").unwrap();
    let mut max = 0;
    for capture in re.captures_iter(&page).map(|c| c.unwrap()) {
        max = capture.get(0).unwrap().range().start;
        if word_type.next_location < max {
            word_type.advance();
        }

        add_vocab(&mut vocab, capture, word_type.name, config.debug);
    }

    vocab = match_extras(vocab, max, &page, &config);

    write_notes(&vocab, &config)
}

fn write_notes(vocab: &Vec<(&str, &str, &str)>, config: &Config) -> Result<(), Box<dyn Error>> {
    println!("Making deck...");
    let mut deck = String::from(
        "#separator:tab
#html:true
#deck column:3
#tags column:4
#notetype:Basic (type in the answer)\n\n",
    );

    let (unit, lesson) = (config.unit, config.lesson);
    for (category, kr, en) in vocab {
        deck.push_str(&format!(
            "{en}\t{kr}\tHTSK::{category}s\thtsk::unit-{unit}::lesson-{lesson} {}\n",
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

//On some pages (i.e. lesson 10) some vocab is in a different format that is the same as
//example sentecnes and other text with attached audio files.
fn match_extras<'a>(
    mut vocab: Vec<(&'a str, &'a str, &'a str)>,
    max: usize,
    page: &'a String,
    config: &Config,
) -> Vec<(&'a str, &'a str, &'a str)> {
    //As I get through more lessons and spot more of these outliers, I will update the following
    //match statement
    //Will return if the lesson doesn't have any specially categorised vocabulary
    let check_max = match config.lesson {
        5 => false,
        10 | 11 => true,
        _ => return vocab,
    };

    println!(
        "\x1b[93mGetting extras...\nThis lesson may require you to alter notes or delete notes which are not vocab!\x1b[0m"
    );

    let re = Regex::new("<a\\N+mp3\">(\\N+)<\\/a>\\N+= (\\D+?)<(?:br \\/|\\/p)>").unwrap();
    for capture in re.captures_iter(page).map(|c| c.unwrap()) {
        let cap = capture.get(0).unwrap();

        if check_max && max < cap.range().start {
            break;
        } else if !cap.as_str().contains("&amp") && (check_max || cap.as_str().contains("Lesson")) {
            add_vocab(&mut vocab, capture, "Other", config.debug);
        }
    }

    vocab
}
