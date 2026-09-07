//! lexopt CLI argument parser
//!
//! ### Architectural Overview
//! - **What it does**: Parses, validates, and sets defaults for all command-line arguments, options, and flags provided to the `pincer` executable.
//! - **How it does**: Utilizes `lexopt::Parser` to stream and parse CLI arguments, setting default directory to `~/Downloads`, default connection split count to 4, and identifying custom RPC ports, secret tokens, and target format flags.
//! - **Where it comes from**: Called by `cli::CliDispatcher::parse_and_dispatch()` at startup.
//! - **Where it leads to**: Returns a strongly typed `CliArgs` struct consumed by `CliDispatcher`, `DaemonManager`, `DirectDownloader`, and `rpc::start_server`.

use lexopt::prelude::*;

/// Strongly-typed structure containing all parsed command-line flags and parameters.
#[derive(Debug, Clone)]
pub struct CliArgs {
    /// If true, detach the current process and run in background.
    pub daemon: bool,
    /// If true, start the WebSocket JSON-RPC server.
    pub enable_rpc: bool,
    /// Custom TCP port to bind for RPC (defaults to 6800).
    pub rpc_listen_port: Option<u16>,
    /// Optional authorization secret token required for RPC method calls.
    pub rpc_secret: Option<String>,
    /// Optional direct download URL positional argument.
    pub url: Option<String>,
    /// Optional custom output filename.
    pub out: Option<String>,
    /// Destination directory for saved downloads (defaults to `~/Downloads`).
    pub dir: String,
    /// Number of concurrent connection threads for downloading (defaults to 4).
    pub split: usize,
    /// If true, display verbose logging / ANSI terminal progress bars.
    pub log: bool,
    /// Optional target format for post-download transcoding (e.g. mp4, mp3, pdf).
    pub format: Option<String>,
    /// Optional speed limit suffix string (e.g. 5M, 500K)
    pub max_download_limit: Option<String>,
}

impl CliArgs {
    /// Parses CLI arguments from the process environment (`std::env::args_os()`).
    ///
    /// Uses `lexopt` for minimal binary overhead and strict POSIX argument validation.
    pub fn parse() -> Result<Self, lexopt::Error> {
        let mut daemon = false;
        let mut enable_rpc = false;
        let mut rpc_listen_port = None;
        let mut rpc_secret = None;
        let mut url = None;
        let mut out = None;

        // Default to ~/Downloads or current directory if HOME is unset.
        let mut dir = std::env::var("HOME")
            .map(|h| format!("{}/Downloads", h))
            .unwrap_or_else(|_| ".".to_string());
        let mut split = 4;
        let mut log = false;
        let mut format = None;
        let mut max_download_limit = None;

        let mut parser = lexopt::Parser::from_env();
        while let Some(arg) = parser.next()? {
            match arg {
                // Daemon flag: -D or --daemon
                Short('D') | Long("daemon") => daemon = true,

                // Enable RPC server flag: --enable-rpc
                Long("enable-rpc") => enable_rpc = true,

                // Custom RPC listener port: -p, --port, --rpc-listen-port <PORT>
                Short('p') | Long("port") | Long("rpc-listen-port") => {
                    rpc_listen_port = Some(parser.value()?.parse()?);
                }

                // RPC authentication secret: --rpc-secret or --secret <SECRET>
                Long("rpc-secret") | Long("secret") => {
                    rpc_secret = Some(parser.value()?.string()?);
                }

                // Custom output filename: -o <NAME> or --out <NAME>
                Short('o') | Long("out") => {
                    out = Some(parser.value()?.string()?);
                }

                // Custom download directory: -d <DIR> or --dir <DIR>
                Short('d') | Long("dir") => {
                    dir = parser.value()?.string()?;
                }

                // Concurrent connection thread count: -s <SPLIT> or --split <SPLIT>
                Short('s') | Long("split") => {
                    split = parser.value()?.parse()?;
                }

                // Interactive terminal progress logging: -l or --log
                Short('l') | Long("log") => log = true,

                // Post-download transcoding target format: -f <FMT> or --format <FMT>
                Short('f') | Long("format") => {
                    format = Some(parser.value()?.string()?);
                }

                // Speed limit: --max-download-limit <LIMIT>
                Long("max-download-limit") | Long("max-overall-download-limit") => {
                    max_download_limit = Some(parser.value()?.string()?);
                }

                // Version flag: -v, -V, --version
                Short('v') | Short('V') | Long("version") => {
                    println!("pincer {}", env!("CARGO_PKG_VERSION"));
                    std::process::exit(0);
                }

                // Help flag: -h, --help
                Short('h') | Long("help") => {
                    println!("Pincer {}", env!("CARGO_PKG_VERSION"));
                    println!("Usage: pincer [OPTIONS] [URL | .torrent | .metalink | magnet:?]");
                    println!();
                    println!("Options:");
                    println!("  -p, --port, --rpc-listen-port <PORT>  Set RPC WebSocket listener port (default: 6800)");
                    println!("      --rpc-secret <SECRET>             Set RPC authentication secret token");
                    println!("  -D, --daemon                          Run as background daemon");
                    println!("      --enable-rpc                      Enable JSON-RPC server mode");
                    println!("  -d, --dir <DIR>                       Download output directory (default: ~/Downloads)");
                    println!("  -o, --out <FILENAME>                  Output filename");
                    println!("  -s, --split <N>                       Number of connection threads (default: 4)");
                    println!("  -f, --format <FMT>                    Transcode output format (mp4, mp3, pdf, etc.)");
                    println!(
                        "      --max-download-limit <SPEED>      Set speed limit (e.g. 5M, 500K)"
                    );
                    println!("  -l, --log                             Enable ANSI terminal progress rendering");
                    println!("  -v, --version                         Print version information");
                    println!("  -h, --help                            Print help message");
                    std::process::exit(0);
                }

                // Positional URL argument
                Value(val) => {
                    if url.is_none() {
                        url = Some(val.string()?);
                    }
                }

                // Unrecognized flags
                _ => return Err(arg.unexpected()),
            }
        }

        Ok(Self {
            daemon,
            enable_rpc,
            rpc_listen_port,
            rpc_secret,
            url,
            out,
            dir,
            split,
            log,
            format,
            max_download_limit,
        })
    }
}
