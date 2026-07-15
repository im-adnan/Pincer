use librqbit::{AddTorrent, Session};
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    println!("Testing librqbit...");
    let session = Session::new(PathBuf::from("/tmp/pincer-test"))
        .await
        .unwrap();

    // Test construction of AddTorrent from url
    let torrent =
        AddTorrent::from_url("magnet:?xt=urn:btih:cab507494d02ebb1178b38f2e9d7be299c86b862");

    // Check if we can add it (don't run it actually, just check if it compiles)
    let res = session.add_torrent(torrent, None).await.unwrap();
    let handle = res.into_handle().unwrap();

    let name = handle.name();
    println!("Torrent name: {:?}", name);
    println!("Info hash: {}", handle.shared.info_hash.as_string());

    let stats = handle.stats();
    println!("Progress bytes: {}", stats.progress_bytes);
}
