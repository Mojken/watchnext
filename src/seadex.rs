use crate::anilist;

use log::{trace, info, error};
use futures::future::join_all;
use tokio::sync::Mutex;
use tokio::task;
use url::Url;
use serde::Deserialize;
use chrono::Utc;
use std::{
    error::Error,
    collections::BTreeMap,
    time::Duration,
    pin::pin,
};

const ENTRY_URL: &str = "https://releases.moe/api/collections/entries/records?filter=alID=";
const TORRENT_URL: &str = "https://releases.moe/api/collections/torrents/records?filter=id=";
const ANILIST_URL: &str = "https://anilist.co/anime";

#[derive(Deserialize, Debug)]
struct FileInfo {
    length: u64,
    name: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct EntryItem {
    collection_id: String,
    collection_name: String,
    created: chrono::DateTime<Utc>,
    updated: chrono::DateTime<Utc>,
    id: String,

    #[serde(rename(deserialize="alID"))]
    al_id: u32,
    comparison: String,
    incomplete: bool,
    notes: String,
    theoretical_best: String,
    trs: Vec<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct TorrentItem {
    collection_id: String,
    collection_name: String,
    created: chrono::DateTime<Utc>,
    updated: chrono::DateTime<Utc>,
    id: String,

    dual_audio: bool,
    files: Vec<FileInfo>,
    grouped_url: String,
    info_hash: String,
    is_best: bool,
    release_group: String,
    tracker: String,
    url: Url,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum SeadexItem {
    Torrent(TorrentItem),
    Entry(EntryItem),
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct SeadexPage {
    page: u8,
    per_page: u8,
    total_items: u8,
    total_pages: u8,
    items: Vec<SeadexItem>,
}

#[derive(Debug)]
struct Torrent {
    size: u64,
    n_files: u16,
    urls: Vec<Url>,
}


struct Client {
    in_tx: tokio::sync::mpsc::Sender::<Url>,
    out_rx: tokio::sync::Mutex<tokio::sync::mpsc::Receiver::<Result<reqwest::Response, reqwest::Error>>>,
}

impl Client {
    pub fn new() -> Self {
        let (in_tx, mut in_rx) =   tokio::sync::mpsc::channel::<Url>(1);
        let (out_tx, out_rx) = tokio::sync::mpsc::channel::<Result<reqwest::Response, reqwest::Error>>(1);

        task::spawn(async move {
            let client: reqwest::Client = reqwest::Client::new();
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(50));

            while let Some(endpoint) = in_rx.recv().await {
                interval.tick().await;

                trace!(target: "seadex", "Sending request to {endpoint}");
                let resp = client.get(endpoint).header("Accept", "application/json").send().await;
                out_tx.send(resp).await;
            }
        });

        Client {
            in_tx: in_tx,
            out_rx: tokio::sync::Mutex::new(out_rx),
        }
    }

    pub async fn send_request(&self, endpoint: Url) -> Result<reqwest::Response, reqwest::Error> {
        let mut lock = self.out_rx.lock().await;

        self.in_tx.send(endpoint).await;
        let resp = lock.recv().await.unwrap();
        return resp;
    }
}

async fn handle_response(res: reqwest::Response) -> Result<String, reqwest::Error> {
    trace!(target: "seadex", "Handling response");
    res.text().await
}

async fn handle_torrent(torrents: &mut BTreeMap<String, Torrent>, resp: String) -> Result<(), Box<dyn Error>> {
    trace!(target: "seadex", "Handling Torrent page");
    let page = match serde_json::from_str::<SeadexPage>(&resp) {
        Ok(page) => page,
        Err(_) => return Ok(()),
    };
    match &page.items[0] {
        SeadexItem::Torrent(item) => {
            if item.tracker == "Nyaa" {
                let torrent: &mut Torrent = torrents.entry(item.release_group.clone()).or_insert(Torrent { size: 0, n_files: 0, urls: Vec::new() });
                for file in &item.files {
                    torrent.size += file.length;
                    if file.name.contains("NC") { continue; };
                    torrent.n_files += 1;
                }
                let mut ret_url: Url = item.url.clone();
                ret_url.set_path(&format!("/download/{}.torrent", item.url.path_segments().expect("URL can be a base").collect::<Vec<&str>>()[1]));
                torrent.urls.push(ret_url);
            }
            return Ok(());
        },
        _ => Err("Got Entry page when expecting Torrent page".into())
    }
}

async fn handle_entry(resp: String) -> Result<Vec<Url>, Box<dyn Error>> {
    trace!(target: "seadex", "Handling Entry page");
    let items: Vec<SeadexItem> = serde_json::from_str::<SeadexPage>(&resp).unwrap().items;
    match &items[..] {
        [] => Err("Entry page missing".into()),
        [item] => {
            match &items[0] {
                SeadexItem::Entry(item) => {
                    return Ok(item.trs.iter().map(|torrent| Url::parse(&format!("{}\"{}\"", TORRENT_URL, torrent)).expect("Url is known to be valid")).collect());
                },
                _ => Err("Got Torrent page when expecting Entry page".into())
            }
        }
        _ => Err("Entry page had too many items. How?".into()),
    }
}

fn handle_torrent_map(media: &anilist::Media, torrents: BTreeMap<String, Torrent>) -> Result<Vec<Url>, Box<dyn Error>> {
    trace!(target: "seadex", "Handling torrent map");

    let mut smallest_torrent_size: u64 = u64::MAX;
    let mut smallest_torrent: Option<Vec<Url>> = None;
    for (_release_group, torrent) in torrents {
        if f32::from(torrent.n_files) < f32::from(media.episodes.expect("Finished series have known episode counts")) * 0.95 { continue };
        if torrent.size < smallest_torrent_size {
            smallest_torrent_size = torrent.size;
            smallest_torrent = Some(torrent.urls);
        }
    }

    match smallest_torrent {
        Some(torrent) => Ok(torrent),
        None => Err(format!("No seadex page found for series {}, id {}", media.title.romaji, media.id).into())
    }
}


async fn do_chain(client: &Client, media: &anilist::Media) -> Option<(u32, Vec<Url>)> {
    match try_do_chain(client, media).await {
        Ok(result) => {
            info!(target: "seadex", "{}: Torrent found", media.id);
            return Some(result);
        },
        Err(e) => error!(target: "seadex", "{}: {} ({})", media.id, e, Url::parse(format!("{}/{}", ANILIST_URL, media.id).as_str()).unwrap()),
    }
    return None;
}

async fn try_do_chain(client: &Client, media: &anilist::Media) -> Result<(u32, Vec<Url>), Box<dyn Error>> {
    let url: Url = Url::parse(&format!("{}{}", ENTRY_URL, media.id)).expect("Url is known to be valid");
    let req: reqwest::Response = client.send_request(url).await?;
    let resp: String = handle_response(req).await?;
    let torrent_urls: Vec<Url> = handle_entry(resp).await?;

    let torrent_resps: Vec<Result<String, reqwest::Error>> = join_all(
        torrent_urls
            .iter()
            .map(|url| async {
                let req: reqwest::Response = client.send_request(url.clone()).await?;
                return handle_response(req).await;
            })
            .collect::<Vec<_>>()
    ).await;

    let mut torrents: BTreeMap<String, Torrent> = BTreeMap::new();
    for maybe_resp in torrent_resps {
        match maybe_resp {
            Ok(resp) => handle_torrent(&mut torrents, resp).await?,
            Err(e) => error!(target: "seadex", "Recieved error when fetching torrent page: {e}")
        }

    }
    Ok((media.id, handle_torrent_map(&media, torrents).unwrap()))
}

pub async fn get_torrents_for_media(series: BTreeMap<u32, &anilist::Media>) -> Vec<(u32, Vec<Url>)> {
    let client = Client::new();

    let series_clone = series.clone();

    trace!(target: "seadex", "Starting stream");
    let results: Vec<Option<(u32, Vec<Url>)>> = join_all(
        series
            .into_iter()
            .map(|(_, media)| do_chain(&client, media))
            .collect::<Vec<_>>()
    ).await;
    trace!(target: "seadex", "Stream finished");

    results.into_iter().flatten().collect()
}
