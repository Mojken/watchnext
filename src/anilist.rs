use serde::Deserialize;
use serde_json::json;
use std::fs::File;
use std::io::BufReader;
use std::io::Write;
use std::error::Error;
use std::collections::BTreeMap;

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
        id
        format
      }
      status
      progress
    }
  }
}
";

#[derive(Deserialize)]
struct AnilistDataWrapper {
    data: Option<AnilistPageWrapper>,
    errors: Option<Vec<ErrorResponse>>,
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

#[derive(Deserialize, Debug)]
struct PageInfo {
    #[serde(rename(deserialize = "hasNextPage"))]
    has_next_page: bool,
    #[serde(rename(deserialize = "lastPage"))]
    last_page: u16,
    total: u16,
}

#[derive(Deserialize, Debug, Clone)]
struct Date {
    day: Option<u8>,
    month: Option<u8>,
    year: Option<u16>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Title {
    pub english: Option<String>,
    native: String,
    pub romaji: String,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MediaFormat {
    Tv,
    TvShort,
    Movie,
    Special,
    Ova,
    Ona,
    Music,
    Manga,
    Novel,
    OneShot,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MediaStatus {
    Finished,
    Releasing,
    NotYetReleased,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Media {
    #[serde(rename(deserialize = "endDate"))]
    anilist_end_date: Date,
    #[serde(skip)]
    pub end_date: Option<chrono::NaiveDate>,
    pub episodes: Option<u16>,
    pub status: MediaStatus,
    pub title: Title,
    pub id: u32,
    pub format: MediaFormat,
}

#[derive(Deserialize, Debug, Clone)]
pub struct MediaProgress {
    pub media: Media,
    pub progress: u16,
    pub status: String,
}

#[derive(Deserialize, Debug)]
pub struct ErrorResponse {
    message: String,
    status: u16,
}


pub fn fetch_from_anilist_spoofed() -> Result<BTreeMap<u32, MediaProgress>, Box<dyn Error>> {
    let file = File::open("test.json")?;
    let mut data: AnilistPageWrapper = serde_json::from_reader::<BufReader<File>, AnilistDataWrapper>(BufReader::new(file)).unwrap().data.unwrap();
    clean_anilist_data(&mut data);
    Ok(data.page.media_list.into_iter().map(|listing| (listing.media.id, listing)).collect())
}

pub async fn fetch_from_anilist() -> Result<BTreeMap<u32, MediaProgress>, Box<dyn Error>> {
    //return fetch_from_anilist_spoofed();

    let client = reqwest::Client::new();
    let mut ret: BTreeMap<u32, MediaProgress> = BTreeMap::new();
    let mut page = 0;

    loop {
        // Define query and variables
        let json = json!({"query": QUERY, "variables": {"username": "Mojken", "page": page}});
        // Make HTTP post request
        let resp = client.post("https://graphql.anilist.co/")
                         .header("Content-Type", "application/json")
                         .header("Accept", "application/json")
                         .body(json.to_string())
                         .send()
                         .await
                         .unwrap()
                         .text()
                         .await?;

        let mut data: AnilistPageWrapper = match serde_json::from_str::<AnilistDataWrapper>(&resp) {
            Ok(data) => match data.errors {
                Some(ref errors) => {
                    let error: &ErrorResponse = errors.first().unwrap();
                    return Err(format!("Anilist returned error code {}: {}", error.status, error.message).into());
                }
                None => {
                    data.data.unwrap()
                }
            },
            Err(e) => {
                return Err(format!("Failed to parse anilist response! Error was: {}  Data was: {}", e, resp).into());
            }
        };

        clean_anilist_data(&mut data);
        ret.append(&mut data.page.media_list.into_iter().map(|listing| (listing.media.id, listing)).collect());

        if !data.page.page_info.has_next_page {
            return Ok(ret)
        }
        page += 1;
    }
}

fn clean_anilist_data(data: &mut AnilistPageWrapper) {
    for media_progress in &mut data.page.media_list {
        match media_progress.media.anilist_end_date.year {
            Some(year) => {
                media_progress.media.end_date = Some(chrono::NaiveDate::from_ymd_opt(
                    year.into(),
                    media_progress.media.anilist_end_date.month.unwrap().try_into().unwrap(),
                    media_progress.media.anilist_end_date.day.unwrap().try_into().unwrap()
                ).expect("Anilist should only serve real dates"));
            },
            None => {
                media_progress.media.end_date = None;
            },
        }
    }
}

async fn parse_anilist() -> Result<(), Box<dyn Error>>{
    let media_list = fetch_from_anilist().await?;

    println!("{:?}", media_list);
    for (_, progress) in media_list {
        match progress.status.as_ref() {
            "CURRENT" =>
                match progress.media.status {
                    MediaStatus::Finished =>
                        println!("Watching: Episode {} of {} (release {:?})", progress.progress, progress.media.title.romaji, progress.media.anilist_end_date.year.ok_or("TBA").unwrap()),
                    MediaStatus::Releasing =>
                        println!("Watching: Episode {} of {} (ongoing)", progress.progress, progress.media.title.romaji),
                    MediaStatus::NotYetReleased =>
                        println!("Somehow watching something unreleased: Episode {} of {}", progress.progress, progress.media.title.romaji),
                }
            "PLANNING" =>
                match progress.media.status {
                    MediaStatus::Finished =>
                        println!("Planning: {} (release {:?})", progress.media.title.romaji, progress.media.anilist_end_date.year.ok_or("TBA").unwrap()),
                    MediaStatus::Releasing =>
                        println!("Planning: {} (ongoing)", progress.media.title.romaji),
                    MediaStatus::NotYetReleased =>
                        println!("Unreleased: {}", progress.media.title.romaji),
                }
            _ =>
                println!(""),
        }
    }
    Ok(())
}
