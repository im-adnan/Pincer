//! Background daemon process spawning & detachment
//!
//! ### Architectural Overview
//! - **What it does**: Detaches the current process and spawns a background `pincer` daemon instance when `--daemon` (`-D`) is passed.
//! - **How it does**: Filters out daemon flags from `std::env::args()`, creates a detached child process using `std::process::Command` with standard I/O redirection to `Stdio::null()`, and prints the spawned child PID.
//! - **Where it comes from**: Called by `cli::CliDispatcher::parse_and_dispatch()` when `args.daemon == true`.
//! - **Where it leads to**: Terminates the foreground terminal process and leaves the daemon child executing in the background.

use std::process::{Command, Stdio};

/// Handles process detachment and background daemon spawning.
pub struct DaemonManager;

impl DaemonManager {
    /// Spawns the current binary as a detached background daemon process.
    ///
    /// The parent process:
    /// 1. Filters out `-D` / `--daemon` arguments to avoid endless recursive spawning in the child.
    /// 2. Disconnects standard input, output, and error handles by mapping them to `Stdio::null()`.
    /// 3. Spawns the detached child process and prints its PID.
    /// 4. Exits with status 0, leaving the background daemon running.
    pub fn spawn_background_process() {
        // Collect current CLI arguments, stripping the daemon flags.
        let args: Vec<String> = std::env::args()
            .filter(|arg| arg != "-D" && arg != "--daemon")
            .collect();

        // Launch detached child process with redirected null streams.
        match Command::new(&args[0])
            .args(&args[1..])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => {
                println!("Pincer started in background (PID: {}).", child.id());
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("Failed to spawn daemon process: {}", e);
                std::process::exit(1);
            }
        }
    }
}
