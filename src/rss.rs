use crate::anilist::MediaStatus;

use std::error::Error;
use rss::Channel;
use chrono::{DateTime, Datelike};
use regex::Regex;
use url::Url;
use priority_queue::PriorityQueue;

/* Consider instead using this generic regex - might be less robust?
 * ^\[(EMBER|SubsPlease|Erai-raws)\] (?P<series>.*?) (\((?P<year>\d{4})\) |\(Season (?P<batch_season>\d)\) |\[1080p.*?\] |S(?P<season>\d\d)E(?P<episode>\d\d) |\[.*?\] ?|\(.*?\) ?|[\(\[](?P<batch>Batch)[\)\]] ?|- (?P<erai_episode>\d\d) |- \d\d ~ \d\d )+(.mkv)?$
 */

// TODO: Add Judas
const NYAA_USERS: [(&str, &str, &str, u16); 4] = [
    (
        "Ember_Encodes",
        r"\[EMBER\] (?P<series>.*) S(?P<season>[0-9]+)E(?P<episode>[0-9]+) \[[0-9]+p\] \[[0-9A-z ]+\] .*",
        r"\[EMBER\] (?P<series>.*) \((?P<year>[0-9]+)\) \(Season (?P<season>\d+)\) .* \(Batch\)",
        20
    ),
    (
        "subsplease",
        r"\[SubsPlease\] (?P<series>.*) - (?P<episode>[0-9]+) \((?P<resolution>[0-9]+p)\) \[.*\](\..*)",
        r"\[SubsPlease\] (?P<series>.*)( -)? \(\d+-\d+\) \((?P<resolution>[0-9]+p)\) \[Batch\]",
        10
    ),
    (
        "Erai-raws",
        r"\[Erai-raws\] (?P<series>.*) - (?P<episode>\d+)(?: \(Repack\))? \[([0-9]+p)\].*\[ENG\].*",
        r"\[Erai-raws\] (?P<series>.*) - (\d+ ~ \d+)(?: \(Repack\))? \[([0-9]+p)\](\[HEVC\])?\[BATCH\].*\[ENG\].*",
        9
    ),
    (
        "HorribleSubs",
        r"\[HorribleSubs\] (?P<series>.*) - (?P<episode>\d+) \[1080p\]\.mkv",
        r"\[HorribleSubs\] (?P<series>.*) \(d+-\d+\) \[1080p\]\ \(Batch\)",
        2
    ),
];

#[derive(PartialEq, Eq, Hash)]
pub struct RSSData {
    pub pub_date: DateTime<chrono::FixedOffset>,
    pub series: String,
    pub episode: i32,
    pub resolution: String,
    pub season: i32,
    pub torrent: Url,
}


pub fn get_torrents_from_rss_feed(rss_url: Url) -> Result<Vec<Url>, Box<dyn Error>> {
    let ret: Vec<Url> = Vec::new();

    let content = reqwest::blocking::get(rss_url)?.bytes()?;

    let channel = Channel::read_from(&content[..])?;
    let rss_data = channel.items();

    for rss_datum in rss_data {
        let pub_date = DateTime::parse_from_rfc2822(rss_datum.pub_date.as_ref().ok_or(std::fmt::Error)?)?;
        let d_release = chrono::Utc::now().signed_duration_since(pub_date);
        println!("{:}", d_release / 60 / 60 / 24);
    }

    return Ok(ret);
}


pub fn get_rss_for_media(series: &crate::anilist::Media) -> Result<(Url, usize), Box<dyn Error>> {
    if series.episodes == Some(1) && series.status == MediaStatus::Finished {
        return Err("TODO: Handle movies".into())
    }

    let fuzzed_titles = fuzzy_search(series);

    let mut pq : PriorityQueue<RSSData, i64> = PriorityQueue::new();

    //TODO: If release date is before 2009, handling this differently may result in better results
    let compare_date: chrono::NaiveDate = if series.status == MediaStatus::Finished {
        series.end_date.expect("Finished series have an end date")
    } else {
        let now = chrono::Utc::now();
        chrono::NaiveDate::from_ymd_opt(now.year(), now.month(), now.day()).expect("Converting from Utc::DateTime to NaiveData should always succeed")
    };

    for (alt, title_weight) in fuzzed_titles.into_sorted_iter() {
        for (nyaa_user_index, nyaa_user) in NYAA_USERS.iter().enumerate() {
            let feed_url = build_nyaa_rss_url(&("\"".to_owned() + &alt.clone() + "\" 1080p"), nyaa_user_index);
            println!("Trying feed {}", feed_url);
            let feed_rss = fetch_rss(&feed_url, nyaa_user_index, true)?;
            for entry in feed_rss {
                let naive_pub_date = chrono::NaiveDate::from_ymd_opt(entry.pub_date.year(), entry.pub_date.month(), entry.pub_date.day()).expect("I trust that pub_date is a valid date");
                let delta_days: f64 = i32::try_from(compare_date.signed_duration_since(naive_pub_date).num_days()).unwrap().into();
                let time_weight: f64 = (1.0 - delta_days/200.0).powf(3.0);
                println!("Entry: Tw {}, uw {}, tw {}", title_weight, nyaa_user.3, time_weight);
                pq.push(entry, (f64::try_from(title_weight).unwrap() * f64::from(nyaa_user.3) * time_weight) as i64);
            }
        }
    }

    if pq.is_empty() {
        Err(format!("No torrents found for series {}", series.title.romaji).into())
    } else {
        println!("{}: {}", series.title.romaji, pq.pop().expect("It's not empty").0.torrent);
        Err(format!("No torrents found for series {}", series.title.romaji).into())
    }
}

pub fn fetch_rss(feed_url: &Url, nyaa_user: usize, airing: bool) -> Result<Vec<RSSData>, Box<dyn Error>> {
    let content = reqwest::blocking::get(feed_url.clone())?.bytes()?;

    match airing {
        true =>
            parse_series_search(Channel::read_from(&content[..])?, Regex::new(NYAA_USERS[nyaa_user].1).unwrap()),
        false => {
            parse_batch_search(Channel::read_from(&content[..])?, Regex::new(NYAA_USERS[nyaa_user].2).unwrap())
        },
    }
}

pub fn build_nyaa_rss_url(series: &str, user: usize) -> Url {
    const NYAA: &'static str = "https://nyaa.si/";
    let mut url = Url::parse(NYAA).expect("URL is known to be valid");
    let query = ["page=rss", "c=0_0", "f=0", &format!("u={}", NYAA_USERS[user].0), &format!("q={series}")].join("&");
    url.set_query(Some(&query));
    url
}

fn parse_batch_search(channel: Channel, re: Regex) -> Result<Vec<RSSData>, Box<dyn Error>> {
    let mut ret: Vec<RSSData> = Vec::new();

    for item in channel.items() {
        let title = item.title.as_ref().ok_or(std::fmt::Error)?.as_str();
        let regex_match = re.captures(title).and_then(|cap| {
            match (
                cap.name("series").map(|part| part.as_str()),
                cap.name("season").map(|part| part.as_str())
            ) {
                (Some(series), Some(season)) => Some((series, season.parse::<i32>().unwrap())),
                (Some(series), None) => Some((series, -1)),
                _ => None,
            }
        });

        match regex_match {
            Some(chunks) => {
                let link = Url::parse(&item.link.as_ref().unwrap()).unwrap();
                let pub_date = DateTime::parse_from_rfc2822(item.pub_date.as_ref().ok_or(std::fmt::Error)?)?;

                ret.push(RSSData {
                    pub_date: pub_date,
                    series: chunks.0.to_string(),
                    season: chunks.1,
                    resolution: "1080p".to_string(),
                    torrent: link,
                    episode: 0,
                });
            },
            _ => {
            }
        }
    }
    Ok(ret)
}

fn parse_series_search(channel: Channel, re: Regex) -> Result<Vec<RSSData>, Box<dyn Error>> {
    let mut ret: Vec<RSSData> = Vec::new();

    for item in channel.items() {
        let title = item.title.as_ref().ok_or(std::fmt::Error)?.as_str();
        let regex_match = re.captures(title).and_then(|cap| {
            match (
                cap.name("series").map(|part| part.as_str()),
                cap.name("episode").map(|part| part.as_str()),
                cap.name("season").map(|part| part.as_str())
            ) {
                (Some(series), Some(episode), Some(season)) => Some((series, episode.parse::<i32>().unwrap(), season.parse::<i32>().unwrap())),
                (Some(series), Some(episode), None) => Some((series, episode.parse::<i32>().unwrap(), -1)),
                _ => None,
            }
        });

        match regex_match {
            Some(chunks) => {
                let link = Url::parse(&item.link.as_ref().unwrap()).unwrap();
                let pub_date = DateTime::parse_from_rfc2822(item.pub_date.as_ref().ok_or(std::fmt::Error)?)?;

                ret.push(RSSData {
                    pub_date: pub_date,
                    series: chunks.0.to_string(),
                    episode: chunks.1,
                    season: chunks.2,
                    resolution: "1080p".to_string(),
                    torrent: link,
                });
            },
            _ => {
            }
        }
    }
    Ok(ret)
}


const FUZZER_OPERATIONS: [(for<'a> fn(&'a str) -> std::string::String, f64); 6] = [
    // Replace ": " with " - "
    (| alt: &str | -> String { alt.replace(": ", " - ") }, 0.95),
    // Remove ": "
    (| alt: &str | -> String { alt.split(": ").collect::<Vec<_>>()[0].to_string() }, 0.5),
    // Remove "'"
    (| alt: &str | -> String { alt.split("'").collect::<Vec<_>>()[0].to_string() }, 0.5),
    // Remove "Part "
    (| alt: &str | -> String { alt.split("Part ").collect::<Vec<_>>()[0].to_string() }, 0.5),
    // Convert all roman numerals to arabic (II -> 2)
    (| alt: &str | -> String {
        let re = Regex::new(r"( |^)([IVX]+)( |$)").unwrap();
        re.replace_all(
            alt,
            |caps: &regex::Captures| {
                format!("{}{}{}",
                        &caps[1],
                        roman::from(&caps[2])
                        .ok_or(format!("Invalid roman {}", &caps[2])).unwrap(),
                        &caps[3]
                )
            }
        ).to_string()
    }, 0.95),
    // Replace any number in string with S<number> (Mushoku Tensei 2 → Mushoku Tensei S2)
    (| alt: &str | -> String {
        let re = Regex::new(r"( |^)([0-9]+)( |$)").unwrap();
        re.replace_all(alt, |caps: &regex::Captures| { format!("{}S{}{}", &caps[1], &caps[2],&caps[3]) }
        ).to_string()
    }, 0.95),
];


pub fn fuzzy_search(series: &crate::anilist::Media) -> PriorityQueue<String, u32> {
    println!("Fuzzing {}", series.title.romaji);
    let mut pq : PriorityQueue<String, u32> = PriorityQueue::new();

    let mut last_pq : PriorityQueue<String, u32> = PriorityQueue::new();
    last_pq.push(series.title.romaji.clone(), 12000000);
    if let Some(title) = &series.title.english {
        last_pq.push(title.to_string(), 10000000);
    }

    let mut next_pq : PriorityQueue<String, u32> = PriorityQueue::new();

    loop {
        for (start, weight) in &last_pq {
            for (operation, op_weight) in FUZZER_OPERATIONS {
                let alt = operation(&start);
                if !alt.eq(start) {
                    next_pq.push_increase(alt, (*weight as f64 * op_weight) as u32);
                }
            }
            pq.push_increase(start.to_string(), *weight);
        }

        if next_pq.len() == 0 {
            break;
        } else {
            last_pq = next_pq;
            next_pq = PriorityQueue::new();
        }
    }

    println!("Generated {} alternatives", pq.len());
    pq
}
