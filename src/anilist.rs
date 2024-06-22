use serde::Deserialize;
use serde_json::json;
use std::fs::File;
use std::io::BufReader;
use std::error::Error;

const QUERY: &str = "
query ($username: String, $page: Int) {
  Page(page: $page, perPage: 50) {
    pageInfo {
      total
      hasNextPage
      lastPage
    }
    mediaList(userName: $username, type: ANIME, status_in: [CURRENT, PLANNING]) {
      media {
        title {
          romaji
          english
          native
        }
        status
        endDate {
          year
          month
          day
        }
        episodes
      }
      status
      progress
    }
  }
}
";

#[derive(Deserialize)]
struct AnilistDataWrapper {
    data: AnilistPageWrapper,
}

#[derive(Deserialize)]
struct AnilistPageWrapper {
    #[serde(rename(deserialize = "Page"))]
    page: AnilistData,
}

#[derive(Deserialize)]
pub struct AnilistData {
    #[serde(rename(deserialize = "mediaList"))]
    pub media_list: Vec<MediaProgress>,
    #[serde(rename(deserialize = "pageInfo"))]
    page_info: PageInfo,
}

#[derive(Deserialize)]
struct PageInfo {
    #[serde(rename(deserialize = "hasNextPage"))]
    has_next_page: bool,
    #[serde(rename(deserialize = "lastPage"))]
    last_page: i8,
    total: i8,
}

#[derive(Deserialize)]
struct Date {
    day: Option<i8>,
    month: Option<i8>,
    year: Option<i16>,
}

#[derive(Deserialize)]
pub struct Title {
    pub english: Option<String>,
    native: String,
    pub romaji: String,
}

#[derive(Deserialize)]
pub struct Media {
    #[serde(rename(deserialize = "endDate"))]
    anilist_end_date: Date,
    #[serde(skip)]
    end_date: chrono::NaiveDate,
    pub episodes: Option<i8>,
    pub status: String,
    pub title: Title,
}

#[derive(Deserialize)]
pub struct MediaProgress {
    pub media: Media,
    pub progress: i8,
    pub status: String,
}

pub fn fetch_from_anilist_spoofed() -> Result<AnilistData, Box<dyn Error>> {
    let file = File::open("test.json")?;
    let ret : AnilistDataWrapper = serde_json::from_reader(BufReader::new(file))?;
    Ok(ret.data.page)
}

fn fetch_from_anilist() -> Result<AnilistData, Box<dyn Error>> {
    let client = reqwest::blocking::Client::new();
    // Define query and variables
    let json = json!({"query": QUERY, "variables": {"username": "Mojken", "page": 0}});
    // Make HTTP post request
    let resp = client.post("https://graphql.anilist.co/")
                     .header("Content-Type", "application/json")
                     .header("Accept", "application/json")
                     .body(json.to_string())
                     .send()
                     .unwrap()
                     .text();

    let ret : AnilistDataWrapper = serde_json::from_str(&resp.unwrap())?;
    Ok(ret.data.page)
}

fn parse_anilist() -> Result<(), Box<dyn Error>>{
    let data = fetch_from_anilist_spoofed()?;

    for progress in data.media_list {
        match progress.status.as_ref() {
            "CURRENT" =>
                match progress.media.status.as_ref() {
                    "FINISHED" =>
                        println!("Watching: Episode {} of {} (release {:?})", progress.progress, progress.media.title.romaji, progress.media.anilist_end_date.year.ok_or("TBA").unwrap()),
                    "RELEASING" =>
                        println!("Watching: Episode {} of {} (ongoing)", progress.progress, progress.media.title.romaji),
                    "NOT_YET_RELEASED" =>
                        println!("Somehow watching something unreleased: Episode {} of {}", progress.progress, progress.media.title.romaji),
                    _ =>
                        println!("Unknown release status"),
                }
            "PLANNING" =>
                match progress.media.status.as_ref() {
                    "FINISHED" =>
                        println!("Planning: {} (release {:?})", progress.media.title.romaji, progress.media.anilist_end_date.year.ok_or("TBA").unwrap()),
                    "RELEASING" =>
                        println!("Planning: {} (ongoing)", progress.media.title.romaji),
                    "NOT_YET_RELEASED" =>
                        println!("Unreleased: {}", progress.media.title.romaji),
                    _ =>
                        println!("Unknown release status"),
                }
            _ =>
                println!(""),
        }
    }
    Ok(())
}
