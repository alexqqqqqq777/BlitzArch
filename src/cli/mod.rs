use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Clone, Debug)]
pub enum Commands {
    /// Create a new archive from specified files and directories.
    #[command(alias = "c")]
    Create {
        /// One or more input files or directories to add to the archive.
        #[arg(required = true)]
        inputs: Vec<PathBuf>,

        /// The path for the output archive file (e.g., my_archive.blz).
        #[arg(short, long)]
        output: PathBuf,

        /// Set a password to encrypt the archive. If not provided, the archive will be unencrypted.
        #[arg(long)]
        password: Option<String>,

        /// Zstandard compression level (0-22). Higher levels offer better compression at the cost of speed.
        #[arg(long, default_value_t = 3)]
        level: i32,

        /// Disable the high-performance Katana format (enabled by default).
        #[arg(long = "no-katana", action = clap::ArgAction::SetFalse, default_value_t = true)]
        katana: bool,

        /// Number of parallel threads to use. [0 = auto-detect based on CPU cores]
        #[arg(long, default_value_t = 0)]
        threads: usize,

        /// Number of threads for the ZSTD (or LZMA2) codec per worker. [0 = auto]
        #[arg(long, default_value_t = 0)]
        codec_threads: u32,

        /// Disable adaptive compression (enabled by default). Adaptive mode stores incompressible chunks without compression to save time.
        #[arg(long = "no-adaptive", action = clap::ArgAction::SetFalse, default_value_t = true)]
        adaptive: bool,

        /// Use the LZMA2 compression algorithm instead of Zstandard.
        #[arg(long)]
        use_lzma2: bool,

        /// LZMA2 compression preset (0-9). Used only with --use-lzma2. [default: 6]
        #[arg(long, value_parser = clap::value_parser!(u32).range(0..=9))]
        lz_level: Option<u32>,

        // --- Deprecated / Advanced --- //
        
        /// `[DEPRECATED]` Use sharded parallel compression mode. The Katana format is recommended instead.
        #[arg(long, hide = true)]
        sharded: bool,

        /// `[ADVANCED]` Target bundle size in MiB for sharded mode.
        #[arg(long, hide = true)]
        bundle_size: u64,

        /// `[ADVANCED]` Strategy for bundling text files to improve compression.
        #[arg(long, value_enum, default_value_t = TextBundleMode::Small, hide = true)]
        text_bundle: TextBundleMode,

        /// `[ADVANCED]` Experimental multi-threaded worker mode.
        #[arg(long, value_enum, default_value_t = WorkerMode::Auto, hide = true)]
        workers: WorkerMode,

        /// `[ADVANCED]` Data compressibility threshold (0.0-1.0) to trigger adaptive store.
        #[arg(long, default_value_t = 0.8, hide = true)]
        adaptive_threshold: f64,
    },

    /// Extract files from an archive.
    #[command(alias = "x")]
    Extract {
        /// The archive file to extract.
        #[arg(required = true)]
        archive: PathBuf,

        /// Specific files or directories to extract. If empty, all files will be extracted.
        files: Vec<PathBuf>,

        /// The directory where files will be extracted. Defaults to the current directory.
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// The password for decrypting the archive. If not provided, will try to read from BLITZARCH_PASSWORD or prompt interactively.
        #[arg(long)]
        password: Option<String>,
    },

    /// List the contents of an archive without extracting it.
    #[command(alias = "l")]
    List {
        /// The archive file to list contents of.
        #[arg(required = true)]
        archive: PathBuf,
    },
}

/// Defines the strategy for bundling text files to improve compression ratios.
#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Eq)]
pub enum TextBundleMode {
    /// Bundle small text files.
    Small,
    /// Automatically determine the best bundling strategy.
    Auto,
    /// Use a sliding window approach for bundling.
    Window,
}

/// Defines the mode for multi-threaded workers.
#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Eq)]
pub enum WorkerMode {
    /// Automatically select the number of workers.
    Auto,
    /// Use 2 worker threads.
    W2,
    /// Use 4 worker threads.
    W4,
}

/// Gets the password from the command-line option, the `BLITZARCH_PASSWORD` environment variable, or prompts the user if necessary.
/// 
/// This function centralizes password retrieval logic.
/// Priority:
/// 1. `--password` command-line argument.
/// 2. `BLITZARCH_PASSWORD` environment variable.
/// 3. Returns `Ok(None)` if neither is present, allowing the caller to prompt interactively.
pub fn get_password_from_opt_or_env(password_opt: Option<String>) -> Result<Option<String>, std::io::Error> {
    if let Some(pass) = password_opt {
        return Ok(Some(pass));
    }
    if let Ok(pass) = std::env::var("BLITZARCH_PASSWORD") {
        return Ok(Some(pass));
    }
    Ok(None)
}

/// Parses command-line arguments using `clap` and returns the command to execute.
///
/// This is the main entry point for the CLI logic.
/// It handles parsing and returns a `Commands` enum variant, or an error if parsing fails.
pub fn run() -> Result<Commands, Box<dyn std::error::Error>> {
    let args = Args::parse();
    Ok(args.command)
}