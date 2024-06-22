mod rss;
mod anilist;

fn main() {
    let data = anilist::fetch_from_anilist_spoofed().expect("IDK man");
    for listing in data.media_list {
        if listing.media.status == "RELEASING" {
            println!("{}:", listing.media.title.romaji);
            rss::get_rss_for_media(&listing.media);
        }
    }
}
