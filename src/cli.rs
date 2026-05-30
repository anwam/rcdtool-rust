use clap::Parser;

/// Command line arguments for rcdtool-rust.
#[derive(Debug, Clone, Parser)]
#[command(
    name = "rcdtool-rust",
    about = "Download Telegram media from private channels via MTProto"
)]
pub struct Arguments {
    /// Message link(s) or .txt file path(s). Use ';' to separate multiple values.
    #[arg(long, default_value = None)]
    pub link: Option<String>,

    /// One or more text files containing message links (one link per line).
    #[arg(long = "link-file", value_delimiter = ';')]
    pub link_files: Vec<String>,

    /// The config filename.
    #[arg(short = 'c', long = "config", default_value = "config.ini")]
    pub config_filename: String,

    /// The channel ID or username.
    #[arg(short = 'C', long = "channel-id", default_value = None)]
    pub channel_id: Option<String>,

    /// The message ID expression. Supports comma-separated values and ranges (`10..15`).
    #[arg(short = 'M', long = "message-id", default_value = None)]
    pub message_id: Option<String>,

    /// Discussion message ID for linked discussion groups.
    #[arg(
        short = 'D',
        long = "discussion-message-id",
        visible_alias = "DM",
        default_value = None
    )]
    pub discussion_message_id: Option<String>,

    /// The output filename.
    #[arg(short = 'O', long = "output", default_value = None)]
    pub output_filename: Option<String>,

    /// Infer extension and rename the output file (enabled by default).
    #[arg(long, default_value_t = true)]
    pub infer_extension: bool,

    /// Rename the file with channel and message IDs.
    #[arg(long, default_value_t = false)]
    pub detailed_name: bool,

    /// Activate dry mode.
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
}

impl Arguments {
    pub fn parse_compat() -> Self {
        let args = std::env::args()
            .map(|arg| match arg.as_str() {
                "-DM" => "--discussion-message-id".to_string(),
                _ => arg,
            })
            .collect::<Vec<_>>();
        Self::parse_from(args)
    }
}
