use fancy_regex::Regex;
use std::{env, error::Error, fs};

pub struct Config {
    pub unit: usize,
    pub lesson: usize,
    pub url: String,
    pub output_dir: std::path::PathBuf,
    pub debug: bool,
}

impl Config {
    pub fn build(args: super::Args) -> Result<Self, &'static str> {
        if args.reset
            && let Err(err) = fs::remove_file("urls.csv")
        {
            eprintln!("Unable to remove urls.csv: {err}");
        }

        let output_dir = if let Ok(path) = env::var("OUTPUT")
            && args.path == "output/"
        {
            path.into()
        } else {
            args.path.into()
        };

        Ok(Config {
            lesson: args.lesson,
            unit: args.lesson / 25,
            url: get_url(&args.lesson)?,
            output_dir,
            debug: args.debug,
        })
    }
}

fn get_url(lesson: &usize) -> Result<String, &'static str> {
    let re = match *lesson {
        92 => Regex::new("100\\/lesson-82\\/"), //typo
        82 => Regex::new("83\\/lesson-82"),     //because of the above typo
        105 => Regex::new("lesson-105a"),
        130 => Regex::new("lesson130"),
        _ => Regex::new(&format!("lesson-{lesson}(-|\\/)")),
    }
    .unwrap();

    for url in get_all_urls()? {
        if re.is_match(&url).unwrap() {
            return Ok("https://www.howtostudykorean.com/".to_string() + &url);
        }
    }
    Err("No lesson URL found")
}

fn get_all_urls() -> Result<Vec<String>, &'static str> {
    if let Ok(contents) = fs::read_to_string("urls.csv") {
        Ok(contents
            .trim_end()
            .split(";")
            .map(|x| x.to_string())
            .collect::<Vec<String>>())
    } else {
        scrape_urls().map_err(|err| {
            println!("{err}");
            "Unable to scrape urls"
        })
    }
}

fn scrape_urls() -> Result<Vec<String>, Box<dyn Error>> {
    println!("Getting all urls...");
    let source = "https://www.howtostudykorean.com/";
    let page = super::fetch_data(&(source.to_string() + "page.xml"))?;

    let re = Regex::new("(?<=<loc>)https:\\S+(?=<\\/loc>)").unwrap();
    let urls = re
        .captures_iter(&page)
        .map(|s| {
            s.unwrap()
                .get(0)
                .unwrap()
                .as_str()
                .trim_start_matches(source)
                .to_string()
        })
        //filters out any urls that include lessons w/o vocab or in languages other than english
        .filter(|x| {
            let mut s = x.split("/");
            s.clone().count() == 4
                && !s.next().unwrap().contains("hanja")
                && !s.nth(1).unwrap().contains("mini-test")
        })
        .collect::<Vec<_>>();

    if let Err(err) = fs::write("urls.csv", urls.join(";")) {
        eprint!("Couldn't write to urls.csv: {err}");
    }

    Ok(urls)
}
