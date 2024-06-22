use error_chain::error_chain;
use rss::Channel;
use chrono::{DateTime, Utc};
use regex::Regex;
use url::Url;
use priority_queue::PriorityQueue;

error_chain! {
    foreign_links {
        TimeParse(chrono::format::ParseError);
        Manual(core::fmt::Error);
        Regex(regex::Error);
        Request(reqwest::Error);
        RSS(rss::Error);
    }
}

const NYAA_USERS: [(&str, &str, i32); 3] = [
    ("Ember_Encodes", r"\[EMBER\] (?P<series>.*) S[0-9]+E(?P<episode>[0-9]+) \[(?P<resolution>[0-9]+p)\] \[[0-9A-z ]+\] .*", 20),
    ("subsplease", r"\[SubsPlease\] (?P<series>.*) - (?P<episode>[0-9]+) \((?P<resolution>[0-9]+p)\) \[.*\](\..*)", 10),
    ("Erai-raws", r"\[Erai-raws\] (?P<series>.*) - (?P<episode>[0-9]+)(?: \(Repack\))? \[([0-9]+p)\].*\[ENG\].*", 9),
];

pub struct RSSData {
    pub_date: DateTime<chrono::FixedOffset>,
    series: String,
    episode: i32,
    resolution: String,
    torrent: Url,
}

pub fn get_rss_for_media(series: &crate::anilist::Media) -> Result<()>{
    //TODO: Care about nyaa user weight
    for (nyaa_user, title_regex, _nyaa_weight) in NYAA_USERS {
        let fuzzed_titles = fuzzy_search(series);
        for (alt, _weight) in fuzzed_titles.into_sorted_iter() {
            let feed_rss = fetch_rss(&alt, nyaa_user, title_regex)?;
            if feed_rss.len() > 0 {
                println!("{}: {}", nyaa_user, build_nyaa_rss_url(&alt, nyaa_user));
                break;
            }
        }
    }
    Ok(())
}


pub fn fetch_rss(title: &str, user: &str, title_regex: &str) -> Result<Vec<RSSData>> {
    let feed_url = build_nyaa_rss_url(title, user);
    let content = reqwest::blocking::get(feed_url)?.bytes()?;

    parse_series_search(Channel::read_from(&content[..])?, Regex::new(title_regex).unwrap())
}

pub fn build_nyaa_rss_url(series: &str, user: &str) -> Url {
    const NYAA: &'static str = "https://nyaa.si/";
    let mut url = Url::parse(NYAA).expect("URL is known to be valid");
    let query = ["page=rss", "c=0_0", "f=0", &format!("u={user}"), &format!("q={series}")].join("&");
    url.set_query(Some(&query));
    url
}

fn parse_series_search(channel: Channel, re: Regex) -> Result<Vec<RSSData>> {
    let now: DateTime<Utc> = Utc::now();
    let mut ret: Vec<RSSData> = Vec::new();

    for item in channel.items() {
        let title = item.title.as_ref().ok_or(std::fmt::Error)?.as_str();
        let regex_match = re.captures(title).and_then(|cap| {
            match (
                cap.name("series").map(|part| part.as_str()),
                cap.name("episode").map(|part| part.as_str()),
                cap.name("resolution").map(|part| part.as_str())
            ) {
                (Some(series), Some(episode), Some(resolution)) => Some((series, episode.parse::<i32>().unwrap(), resolution)),
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
                    resolution: chunks.2.to_string(),
                    torrent: link,
                });
            },
            _ => {
            }
        }
    }
    Ok(ret)
}


const FUZZER_OPERATIONS: [(for<'a> fn(&'a str) -> std::string::String, f64); 5] = [
    (| alt: &str | -> String { alt.replace(": ", " - ") }, 0.95),
    (| alt: &str | -> String { alt.split(": ").collect::<Vec<_>>()[0].to_string() }, 0.5),
    (| alt: &str | -> String { alt.split("Part ").collect::<Vec<_>>()[0].to_string() }, 0.5),
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
    (| alt: &str | -> String {
        let re = Regex::new(r"( |^)([0-9]+)( |$)").unwrap();
        re.replace_all(alt, |caps: &regex::Captures| { format!("{}S{}{}", &caps[1], &caps[2],&caps[3]) }
        ).to_string()
    }, 0.95),
];


pub fn fuzzy_search(series: &crate::anilist::Media) -> PriorityQueue<String, u64> {
    let mut pq : PriorityQueue<String, u64> = PriorityQueue::new();

    let mut last_pq : PriorityQueue<String, u64> = PriorityQueue::new();
    last_pq.push(series.title.romaji.clone(), 120000000);
    if series.title.english.is_some() {
        last_pq.push(series.title.english.clone().expect("I just checked"), 100000000);
    }

    let mut next_pq : PriorityQueue<String, u64> = PriorityQueue::new();

    loop {
        for (start, weight) in &last_pq {
            for (operation, op_weight) in FUZZER_OPERATIONS {
                let alt = operation(&start);
                if !alt.eq(start) {
                    next_pq.push_increase(alt, (*weight as f64 * op_weight) as u64);
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

    pq
}
