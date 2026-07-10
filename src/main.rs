use clap::Parser;
use blog_autopost_rs::config::{self, parse_config};
use std::fs;

mod cli;
mod commands;

use cli::Cli;

/// SIGPIPE をデフォルト(プロセス終了)に戻す。
///
/// Rust は既定で SIGPIPE を無視するため、`blog-autopost-rs check | head` の
/// ように出力先パイプが途中で閉じると、以降の write が EPIPE を返し、
/// `println!` がそれを unwrap して panic する。ここで既定挙動へ戻すことで、
/// パイプ切断時は静かに終了するようにする。
#[cfg(unix)]
fn reset_sigpipe() {
    // SAFETY: プログラム起動直後に一度だけ呼び、既定のシグナル動作へ戻すのみ。
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

#[cfg(not(unix))]
fn reset_sigpipe() {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    reset_sigpipe();

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
