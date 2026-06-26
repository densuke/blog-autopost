use clap::Parser;
use blog_autopost_rs::config::{self, parse_config};
use std::fs;

mod cli;
mod commands;

use cli::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut cli = Cli::parse();

    // config.ymlの読み込み
    let config_content = fs::read_to_string(&cli.config).unwrap_or_else(|_| "".to_string());
    let config_data = parse_config(&config_content).unwrap_or_else(|_| config::Config {
        announcement_text: None,
        blog: None,
        sns: vec![],
        templates: Default::default(),
        default_allowed_timings: None,
        allowed_timings_tolerance_minutes: None,
        allowed_timings: None,
        web_auth: None,
        extra: Default::default(),
    });

    if cli.list_sns {
        commands::list_sns(&config_data);
        return Ok(());
    }

    if cli.list_feeds {
        commands::list_feeds(&config_data);
        return Ok(());
    }

    // サブコマンドを取り出す。指定が無い場合はヘルプを表示して終了する。
    let command = match cli.command.take() {
        Some(cmd) => cmd,
        None => {
            use clap::CommandFactory;
            Cli::command().print_help()?;
            println!();
            return Ok(());
        }
    };

    commands::run_command(command, config_data, &cli).await
}
