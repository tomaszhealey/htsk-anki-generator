use fancy_regex::Regex;
use std::{error::Error, fs};

pub struct Config {
    pub unit: usize,
    pub lesson: usize,
    pub url: String,
}

impl Config {
    pub fn build(mut args: impl Iterator<Item = String>) -> Result<Self, &'static str> {
        //first argument is the path
        args.next();

        let lesson = args
            .next()
            .ok_or("Didn't get a lesson")?
            .parse::<usize>()
            .map_err(|_| "Unable to parse lesson argument")?;

        let unit = lesson / 25;

        Ok(Config {
            unit,
            lesson,
            url: get_url(&lesson)?,
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
            return Ok(url);
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
    let page = super::fetch_data(source.to_string() + "page.xml")?;

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
