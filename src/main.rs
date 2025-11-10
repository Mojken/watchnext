mod rss;
mod anilist;
mod seadex;

use reqwest::Url;
use tokio;
use serde::{Serialize, Deserialize};
use std::collections::BTreeMap;
use std::io::BufWriter;
use std::fs::File;

use qbit_rs::Qbit;
use qbit_rs::model::{AddTorrentArg, TorrentSource};

#[derive(Serialize, Deserialize, Default)]
pub struct SeriesData {
    id: u32,
    title: String,
    downloaded: u16,
    active: bool,
    rss: Option<Url>,
    seen_urls: Vec<Url>,
}

async fn get_torrent_urls_for_series(anilist_data: &BTreeMap<u32, anilist::MediaProgress>, series_data: &mut BTreeMap<u32, SeriesData>) -> Vec<(u32, Vec<Url>)> {
    let seadex_results: Vec<(u32, Vec<Url>)> = seadex::get_torrents_for_media(anilist_data.into_iter().filter(|(_, listing)| listing.media.status == anilist::MediaStatus::Finished).map(|(id, x)| return (*id, &x.media)).collect()).await;
    for (id, _) in &seadex_results {
        series_data.get_mut(&id).unwrap().downloaded = anilist_data.get(&id).unwrap().media.episodes.expect("Finished show didn't have an episode count");
    }
    return seadex_results;
}

fn read_savefile<P: std::convert::AsRef<std::path::Path>>(savefile: P) -> BTreeMap<u32, SeriesData> {
    let mut series_data: BTreeMap<u32, SeriesData> = BTreeMap::new();

    match std::fs::read_to_string(savefile) {
        Ok(content) => {
            println!("Savefile found.\nWelcome back to WatchNext!");
            let save_data: Vec<SeriesData> = serde_json::from_str(&content).unwrap();
            for series_datum in save_data {
                series_data.insert(series_datum.id, series_datum);
            };
        }
        _ => {
            println!("No savefile found, creating one.\nWelcome to WatchNext!");
        }
    }

    return series_data;
}

fn update_savefile(series_data: &mut BTreeMap<u32, SeriesData>, anilist_data: &BTreeMap<u32, anilist::MediaProgress>) {
    // Mark all no-longer-present series as inactive
    for (_id, series) in &mut *series_data {
        series.active = false;
    }

    for (id, listing) in anilist_data {
        let series_datum: &mut SeriesData = series_data.entry(listing.media.id).or_insert(
            SeriesData {
                id: *id,
                title: listing.media.title.romaji.clone(),
                ..Default::default()
            }
        );
        if series_datum.downloaded < listing.media.episodes.unwrap_or(0) {
            series_datum.active = true;
        }
    }
}

async fn add_to_qbit(torrent_urls: BTreeMap<u32, Vec<Url>>, series_data: &mut BTreeMap<u32, SeriesData>, anilist_data: &BTreeMap<u32, anilist::MediaProgress>) {
    let qapi = Qbit::builder()
        .endpoint("http://localhost:9097")
        .cookie("")
        .build();

    for (id, urls) in torrent_urls {
        let series_datum: &mut SeriesData = series_data.get_mut(&id).expect("Savefile should contain all relevant ids");
        let save_location: String = format!("/anime/{}/{}", match &anilist_data.get(&id).unwrap().media.format {
            anilist::MediaFormat::Movie => {
                "movies"
            },
            anilist::MediaFormat::Music => {
                "music"
            },
            _ => {
                "anime"
            }
        }, series_datum.title);

        match qapi.add_torrent(
            AddTorrentArg::builder()
                .source(TorrentSource::Urls { urls: urls.clone().into() })
                .savepath(save_location)
                //.root_folder("false".to_string()) Does not work
                //.paused("true".to_string()) Does not work
                .category("WatchNext".to_string())
                .build()
        ).await {
            Ok(()) => {series_datum.seen_urls.append(&mut urls.clone());},
            Err(e) => {println!("Failed to add torrent to qBittorrent: {}", e);}
        }
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let savefile = match std::env::home_dir() {
        Some(mut path) => {
            path.push(".local/share/watchnext/savefile.json");
            path
        },
        None => {
            println!("Failed to identify home directory, can't locate savefile. Exiting.");
            std::process::exit(0);
        }
    };
    let mut series_data: BTreeMap<u32, SeriesData> = read_savefile(&savefile);

    let anilist_data: BTreeMap<u32, anilist::MediaProgress> = anilist::fetch_from_anilist().await.expect("Failed to fetch from anilist");
    update_savefile(&mut series_data, &anilist_data);

    let mut torrent_urls: BTreeMap<u32, Vec<Url>> = BTreeMap::new();

    let binding = anilist_data.clone().into_iter().filter(|(id, _)| series_data.get(&id).unwrap().active).collect();
    let rets = get_torrent_urls_for_series(&binding, &mut series_data).await;
    for (id, urls) in rets {
        torrent_urls.insert(id, urls.to_vec());
    }

    //add_to_qbit(torrent_urls, &mut series_data, &anilist_data).await;

    std::fs::create_dir_all(&savefile.parent().unwrap()).unwrap();
    let writer = BufWriter::new(File::create(&savefile).expect("Fuck, failed to create savefile. Data loss likely, though recoverable."));
    match serde_json::to_writer_pretty(writer, &series_data.values().collect::<Vec<&SeriesData>>()) {
        Ok(_) => {
        }
        Err(e) =>
            println!("Failed to save {savefile:?}: {e}"),
    }
}
