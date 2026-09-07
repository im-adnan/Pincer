# Pincer User Guide

Welcome to the Pincer Engine! This guide will walk you through how to start downloading files quickly and easily using the Command Line Interface (CLI). 

If you are a developer looking to integrate Pincer into your own app or use the advanced API, check out the [API docs](api/01-connecting.md). For the full CLI flag reference, see [CLI Reference](03-cli-reference.md).

---

## 1. Downloading a Standard File
To download a file from the internet, you just need its URL (the web address, like `https://example.com/movie.mp4`).

Open your terminal and type:
```bash
./target/release/pincer "https://example.com/movie.mp4" --log
```
Pincer will automatically split the file and start downloading it as fast as possible. By passing the `--log` or `-l` flag, you'll see a live progress bar on your screen showing the speed, time remaining, and the percentage completed.

### Choosing Where to Save
By default, Pincer saves the file in the folder where you ran the command. If you want to save it somewhere else (like your Downloads folder), use the `--dir` option:
```bash
./target/release/pincer "https://example.com/movie.mp4" --dir ~/Downloads --log
```

---

## 2. Speeding It Up (Or Slowing It Down)
If you are on a slow connection or a shared network, you might not want Pincer to use all of the available internet speed. 

You can limit the speed using the `--max-download-limit` option (e.g., `5M` for 5 Megabytes per second):
```bash
./target/release/pincer "https://example.com/movie.mp4" --max-download-limit 5M --log
```

If you want to speed things up by using even more "trucks" (concurrent connections), you can change the `--split` option. The default is 4, but you can increase it up to 16:
```bash
./target/release/pincer "https://example.com/movie.mp4" --split 16 --log
```
*(Note: Setting this too high might cause the server to temporarily block you, so 4 to 8 is usually a safe sweet spot!)*

---

## 3. Downloading BitTorrent (Magnet Links)
Pincer has a powerful, built-in BitTorrent engine. To download a torrent, you don't need any special settings. Just pass the Magnet link (which usually starts with `magnet:?xt=...`) just like a normal URL:

```bash
./target/release/pincer "magnet:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c&dn=Ubuntu" --dir ~/Downloads --log
```
Pincer will automatically connect to the BitTorrent network, find neighbors who have the file, and download it for you. 

---

## 4. Pausing and Resuming
If your internet drops or you need to turn off your computer, don't worry! 

Pincer creates a special `.download` file while it is working. If you cancel the download (by pressing `Ctrl+C` in your terminal) or if it gets interrupted, simply run the **exact same command** again later. Pincer will see the `.download` file, figure out exactly where it left off, and resume without losing any progress!

---

## 5. Converting Files Automatically
If you are downloading a video or an image and want it in a different format, you can tell Pincer to convert it the moment the download finishes using the `--format` (or `-f`) option:

```bash
./target/release/pincer "https://example.com/image.png" --format jpg --log
```
Pincer will download the `PNG` image, and then automatically convert it into a `JPG` for you!

---

## Need More Help?
If you ever forget a command, you can ask Pincer for help by typing:
```bash
./target/release/pincer --help
```
This will print out a list of all the cool things Pincer can do!
