//! MCP の接続設定を各クライアントへ登録する。
//!
//! クライアントごとに設定ファイルの場所と形式が違うため、URL とキーを
//! 受け取って適切な形へ変換する。判断のいる処理はすべて純粋な関数へ
//! 切り出してあり、ファイル操作やコマンド実行と分けて検証できる。

use anyhow::{Context, Result, anyhow};
use blog_autopost_rs::config::Config;
use std::path::{Path, PathBuf};

/// MCP エンドポイントのパス。
const MCP_ENDPOINT_PATH: &str = "/api/mcp";

/// 設定を書き込む対象のクライアント。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpClient {
    /// Claude Code。`claude mcp add` を通して登録する。
    Claude,
    /// Codex。`~/.codex/config.toml` を直接書き換える。
    Codex,
}

impl McpClient {
    /// 文字列から対象を決める。
    pub fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "claude" | "claude-code" => Ok(McpClient::Claude),
            "codex" => Ok(McpClient::Codex),
            other => Err(anyhow!(
                "Unknown client: {}. Supported: claude, codex",
                other
            )),
        }
    }

    /// 人が読む表示名を返す。
    pub fn label(&self) -> &'static str {
        match self {
            McpClient::Claude => "Claude Code",
            McpClient::Codex => "Codex",
        }
    }
}

/// 与えられた URL から MCP エンドポイントの URL を組み立てる。
///
/// ベース URL を渡された場合は `/api/mcp` を付け足す。すでに
/// エンドポイントまで含んでいる場合はそのまま使う。
/// 末尾のスラッシュは取り除く。
pub fn build_endpoint_url(input: &str) -> Result<String> {
    let trimmed = input.trim().trim_end_matches('/');

    if trimmed.is_empty() {
        return Err(anyhow!("URL must not be empty"));
    }

    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return Err(anyhow!(
            "URL must start with http:// or https://: {}",
            input.trim()
        ));
    }

    if trimmed.ends_with(MCP_ENDPOINT_PATH) {
        return Ok(trimmed.to_string());
    }

    // 旧トランスポートの URL を渡された場合は、新しい方へ寄せる
    if let Some(base) = trimmed.strip_suffix("/api/mcp/sse") {
        return Ok(format!("{}{}", base, MCP_ENDPOINT_PATH));
    }

    Ok(format!("{}{}", trimmed, MCP_ENDPOINT_PATH))
}

/// 設定から MCP 用の API キーを取り出す。
///
/// `mcp.api_key` を優先し、無ければ `web_auth.secret_key` を使う。
/// これは認証側の判定 (`McpAuthPolicy`) と同じ順序である。
pub fn resolve_api_key(config: &Config) -> Result<String> {
    let mcp_key = config
        .mcp
        .as_ref()
        .and_then(|m| m.api_key.as_deref())
        .filter(|k| !k.is_empty());

    if let Some(key) = mcp_key {
        return Ok(key.to_string());
    }

    let web_key = config
        .web_auth
        .as_ref()
        .and_then(|a| a.secret_key.as_deref())
        .filter(|k| !k.is_empty());

    match web_key {
        Some(key) => {
            println!(
                "Note: using web_auth.secret_key because mcp.api_key is not set. \
                 Consider setting a dedicated key so that a leak stays limited to MCP."
            );
            Ok(key.to_string())
        }
        None => Err(anyhow!(
            "No API key found. Set mcp.api_key in your config.yml.\n\
             Generate one with: openssl rand -hex 32"
        )),
    }
}

/// `claude mcp add` に渡す引数を組み立てる。
///
/// 実行はせず、引数の並びだけを返す。テストから内容を確認できる。
pub fn claude_add_args(name: &str, endpoint: &str, api_key: &str, scope: &str) -> Vec<String> {
    vec![
        "mcp".to_string(),
        "add".to_string(),
        "--transport".to_string(),
        "http".to_string(),
        "--scope".to_string(),
        scope.to_string(),
        name.to_string(),
        endpoint.to_string(),
        "--header".to_string(),
        format!("X-Api-Key: {}", api_key),
    ]
}

/// `claude mcp remove` に渡す引数を組み立てる。
pub fn claude_remove_args(name: &str, scope: &str) -> Vec<String> {
    vec![
        "mcp".to_string(),
        "remove".to_string(),
        "--scope".to_string(),
        scope.to_string(),
        name.to_string(),
    ]
}

/// 引数の並びを、人が貼り付けられるコマンド文字列へ整える。
///
/// 空白を含む引数は引用する。キーが混じるため、表示する側で
/// 伏せるかどうかを判断できるよう組み立てだけを担う。
pub fn format_command(program: &str, args: &[String]) -> String {
    let mut out = String::from(program);
    for arg in args {
        out.push(' ');
        if arg.contains(' ') {
            out.push_str(&format!("'{}'", arg));
        } else {
            out.push_str(arg);
        }
    }
    out
}

/// Codex の設定ファイルへ MCP サーバの定義を追記する。
///
/// 既存の内容とコメントは保つ。同じ名前の定義があれば置き換える。
/// キーは書き込まず、環境変数名だけを記録する
/// (Codex が `bearer_token` を受け付けないため)。
pub fn upsert_codex_entry(
    existing: &str,
    name: &str,
    endpoint: &str,
    key_env_var: &str,
) -> Result<String> {
    let mut doc = existing
        .parse::<toml_edit::DocumentMut>()
        .context("Failed to parse the Codex config as TOML")?;

    // mcp_servers そのものが無ければ作る
    if !doc.contains_key("mcp_servers") {
        doc["mcp_servers"] = toml_edit::Item::Table(toml_edit::Table::new());
    }

    let servers = doc["mcp_servers"]
        .as_table_mut()
        .ok_or_else(|| anyhow!("mcp_servers is not a table in the Codex config"))?;
    // 個々のサーバ定義を [mcp_servers.name] の形で書き出させる
    servers.set_implicit(true);

    let mut entry = toml_edit::Table::new();
    entry["url"] = toml_edit::value(endpoint);
    entry["bearer_token_env_var"] = toml_edit::value(key_env_var);
    servers[name] = toml_edit::Item::Table(entry);

    Ok(doc.to_string())
}

/// Codex の設定ファイルから MCP サーバの定義を取り除く。
///
/// 取り除いたかどうかを第2要素で返す。他の定義には触れない。
pub fn remove_codex_entry(existing: &str, name: &str) -> Result<(String, bool)> {
    let mut doc = existing
        .parse::<toml_edit::DocumentMut>()
        .context("Failed to parse the Codex config as TOML")?;

    let Some(servers) = doc
        .get_mut("mcp_servers")
        .and_then(|item| item.as_table_mut())
    else {
        return Ok((existing.to_string(), false));
    };

    let removed = servers.remove(name).is_some();

    // 空になった mcp_servers を残すと設定不備に見えるため片付ける
    if servers.is_empty() {
        doc.remove("mcp_servers");
    }

    Ok((doc.to_string(), removed))
}

/// Codex の設定ファイルの場所を返す。
fn codex_config_path() -> Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("HOME is not set, cannot locate ~/.codex/config.toml"))?;
    Ok(home.join(".codex").join("config.toml"))
}

/// `install` / `uninstall` / `print` の共通引数。
#[derive(Debug, Clone)]
pub struct McpSetupOptions {
    /// 対象クライアント。
    pub client: McpClient,
    /// 接続先の URL。ベース URL でもエンドポイントまででもよい。
    pub url: Option<String>,
    /// 登録するサーバ名。
    pub name: String,
    /// Claude Code のスコープ。
    pub scope: String,
    /// Codex がキーを読む環境変数の名前。
    pub key_env_var: String,
    /// 実際には変更せず、行う内容だけを表示する。
    pub dry_run: bool,
}

/// 接続設定をクライアントへ登録する。
pub fn install(config: &Config, options: &McpSetupOptions) -> Result<()> {
    let endpoint = require_url(options)?;
    let api_key = resolve_api_key(config)?;

    match options.client {
        McpClient::Claude => install_claude(options, &endpoint, &api_key),
        McpClient::Codex => install_codex(options, &endpoint, &api_key),
    }
}

/// 登録済みの接続設定を取り除く。
pub fn uninstall(options: &McpSetupOptions) -> Result<()> {
    match options.client {
        McpClient::Claude => uninstall_claude(options),
        McpClient::Codex => uninstall_codex(options),
    }
}

/// 書き込まずに設定内容だけを表示する。
pub fn print(config: &Config, options: &McpSetupOptions) -> Result<()> {
    let endpoint = require_url(options)?;
    let api_key = resolve_api_key(config)?;

    match options.client {
        McpClient::Claude => {
            println!("# {} — run this command", options.client.label());
            println!(
                "{}",
                format_command(
                    "claude",
                    &claude_add_args(&options.name, &endpoint, &api_key, &options.scope),
                )
            );
            println!();
            println!("# or write this into .mcp.json / ~/.claude.json");
            println!(
                "{}",
                claude_json_snippet(&options.name, &endpoint, &api_key)
            );
        }
        McpClient::Codex => {
            println!(
                "# {} — add this to ~/.codex/config.toml",
                options.client.label()
            );
            print!(
                "{}",
                upsert_codex_entry("", &options.name, &endpoint, &options.key_env_var)?
            );
            println!();
            print_codex_env_hint(&options.key_env_var, &api_key);
        }
    }

    Ok(())
}

/// Claude Code 向けの JSON 断片を組み立てる。
pub fn claude_json_snippet(name: &str, endpoint: &str, api_key: &str) -> String {
    let value = serde_json::json!({
        "mcpServers": {
            name: {
                "type": "http",
                "url": endpoint,
                "headers": { "X-Api-Key": api_key }
            }
        }
    });
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string())
}

/// URL が指定されていることを確かめ、エンドポイントを組み立てる。
fn require_url(options: &McpSetupOptions) -> Result<String> {
    let url = options
        .url
        .as_deref()
        .ok_or_else(|| anyhow!("--url is required (for example: https://autopost.example.com)"))?;
    build_endpoint_url(url)
}

/// Claude Code へ登録する。
fn install_claude(options: &McpSetupOptions, endpoint: &str, api_key: &str) -> Result<()> {
    let args = claude_add_args(&options.name, endpoint, api_key, &options.scope);

    if options.dry_run {
        println!("[dry-run] would run:");
        println!("  {}", format_command("claude", &args));
        return Ok(());
    }

    run_claude(&args).map(|_| {
        println!(
            "Registered '{}' with {} (scope: {}).",
            options.name,
            options.client.label(),
            options.scope
        );
        println!("  endpoint: {}", endpoint);
        println!("Verify with: claude mcp list");
    })
}

/// Claude Code から取り除く。
fn uninstall_claude(options: &McpSetupOptions) -> Result<()> {
    let args = claude_remove_args(&options.name, &options.scope);

    if options.dry_run {
        println!("[dry-run] would run:");
        println!("  {}", format_command("claude", &args));
        return Ok(());
    }

    run_claude(&args).map(|_| {
        println!(
            "Removed '{}' from {} (scope: {}).",
            options.name,
            options.client.label(),
            options.scope
        );
    })
}

/// `claude` コマンドを実行する。
///
/// 見つからない場合は、手で実行できるコマンドを添えてエラーにする。
fn run_claude(args: &[String]) -> Result<()> {
    let output = std::process::Command::new("claude").args(args).output();

    let output = match output {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(anyhow!(
                "The 'claude' command was not found on PATH.\n\
                 Run this yourself once Claude Code is available:\n  {}",
                format_command("claude", args)
            ));
        }
        Err(e) => return Err(anyhow!("Failed to run the 'claude' command: {}", e)),
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!(
            "The 'claude' command failed: {}\n{}",
            output.status,
            stderr.trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.trim().is_empty() {
        println!("{}", stdout.trim());
    }
    Ok(())
}

/// Codex の設定ファイルへ登録する。
fn install_codex(options: &McpSetupOptions, endpoint: &str, api_key: &str) -> Result<()> {
    let path = codex_config_path()?;
    let existing = read_codex_config(&path)?;
    let updated = upsert_codex_entry(&existing, &options.name, endpoint, &options.key_env_var)?;

    if options.dry_run {
        println!("[dry-run] would write {}:", path.display());
        print!("{}", updated);
        println!();
        print_codex_env_hint(&options.key_env_var, api_key);
        return Ok(());
    }

    write_codex_config(&path, &updated)?;

    println!(
        "Registered '{}' with {} in {}.",
        options.name,
        options.client.label(),
        path.display()
    );
    println!("  endpoint: {}", endpoint);
    println!();
    print_codex_env_hint(&options.key_env_var, api_key);

    Ok(())
}

/// Codex の設定ファイルから取り除く。
fn uninstall_codex(options: &McpSetupOptions) -> Result<()> {
    let path = codex_config_path()?;
    let existing = read_codex_config(&path)?;
    let (updated, removed) = remove_codex_entry(&existing, &options.name)?;

    if !removed {
        println!(
            "'{}' was not found in {}. Nothing to do.",
            options.name,
            path.display()
        );
        return Ok(());
    }

    if options.dry_run {
        println!("[dry-run] would write {}:", path.display());
        print!("{}", updated);
        return Ok(());
    }

    write_codex_config(&path, &updated)?;
    println!("Removed '{}' from {}.", options.name, path.display());
    println!(
        "The {} environment variable is no longer needed.",
        options.key_env_var
    );

    Ok(())
}

/// Codex の設定ファイルを読む。存在しない場合は空として扱う。
fn read_codex_config(path: &Path) -> Result<String> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(anyhow!("Failed to read {}: {}", path.display(), e)),
    }
}

/// Codex の設定ファイルを書く。親ディレクトリが無ければ作る。
fn write_codex_config(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    std::fs::write(path, contents).with_context(|| format!("Failed to write {}", path.display()))
}

/// Codex 用に環境変数の設定方法を案内する。
///
/// Codex は `bearer_token` を受け付けないため、キーは設定ファイルへ
/// 書かず環境変数から読ませる。シェルの設定ファイルは書き換えない。
fn print_codex_env_hint(key_env_var: &str, api_key: &str) {
    println!("Codex reads the key from an environment variable, not from the config file.");
    println!("Add this to your shell profile (this tool does not edit it for you):");
    println!();
    println!("  export {}='{}'", key_env_var, api_key);
}

#[cfg(test)]
mod tests {
    use super::*;
    use blog_autopost_rs::config::{McpConfig, WebAuthConfig};
    use std::collections::HashMap;

    /// 検証用の設定を作る。
    fn config_with(secret_key: Option<&str>, mcp_api_key: Option<&str>) -> Config {
        Config {
            announcement_text: None,
            blog: None,
            sns: vec![],
            templates: HashMap::new(),
            default_allowed_timings: None,
            allowed_timings_tolerance_minutes: None,
            allowed_timings: None,
            web_auth: Some(WebAuthConfig {
                username: "admin".to_string(),
                password: "hashed".to_string(),
                secret_key: secret_key.map(|s| s.to_string()),
                session_ttl_hours: None,
                cookie_secure: None,
                login_max_attempts: None,
                login_window_seconds: None,
            }),
            mcp: mcp_api_key.map(|k| McpConfig {
                api_key: Some(k.to_string()),
                ..Default::default()
            }),
            extra: HashMap::new(),
        }
    }

    // --- CLI の既定値 ---

    #[test]
    fn 既定値が期待どおりに解釈される() {
        use clap::Parser;

        let cli = crate::cli::Cli::parse_from([
            "blog-autopost-rs",
            "mcp",
            "install",
            "--client",
            "codex",
        ]);

        let Some(crate::cli::Commands::Mcp {
            action: crate::cli::McpAction::Install(args),
        }) = cli.command
        else {
            panic!("mcp install として解釈されるはず");
        };

        assert_eq!(args.name, crate::cli::DEFAULT_MCP_SERVER_NAME);
        assert_eq!(args.key_env_var, crate::cli::DEFAULT_MCP_KEY_ENV_VAR);
        assert_eq!(args.scope, crate::cli::DEFAULT_MCP_SCOPE);
        // 既定では何も書き換えない指定になっていないこと
        assert!(!args.dry_run);
    }

    // --- クライアントの判別 ---

    #[test]
    fn クライアント名を解釈できる() {
        assert_eq!(McpClient::parse("claude").unwrap(), McpClient::Claude);
        assert_eq!(McpClient::parse("claude-code").unwrap(), McpClient::Claude);
        assert_eq!(McpClient::parse("codex").unwrap(), McpClient::Codex);
        // 大文字小文字と前後の空白は無視する
        assert_eq!(McpClient::parse(" CODEX ").unwrap(), McpClient::Codex);
    }

    #[test]
    fn 未知のクライアント名はエラーになる() {
        let err = McpClient::parse("agy").unwrap_err();

        assert!(err.to_string().contains("Unknown client"));
        // 使える名前を示す
        assert!(err.to_string().contains("claude"));
        assert!(err.to_string().contains("codex"));
    }

    // --- URL の組み立て ---

    #[test]
    fn ベースurlにエンドポイントを足す() {
        assert_eq!(
            build_endpoint_url("https://autopost.example.com").unwrap(),
            "https://autopost.example.com/api/mcp"
        );
    }

    #[test]
    fn 末尾のスラッシュを取り除く() {
        assert_eq!(
            build_endpoint_url("https://autopost.example.com/").unwrap(),
            "https://autopost.example.com/api/mcp"
        );
    }

    #[test]
    fn エンドポイントまで渡されても重複させない() {
        assert_eq!(
            build_endpoint_url("https://autopost.example.com/api/mcp").unwrap(),
            "https://autopost.example.com/api/mcp"
        );
        assert_eq!(
            build_endpoint_url("https://autopost.example.com/api/mcp/").unwrap(),
            "https://autopost.example.com/api/mcp"
        );
    }

    #[test]
    fn 旧sseのurlは新しい方へ寄せる() {
        // ドキュメントを見て古い URL を渡してしまう場合を拾う
        assert_eq!(
            build_endpoint_url("https://autopost.example.com/api/mcp/sse").unwrap(),
            "https://autopost.example.com/api/mcp"
        );
    }

    #[test]
    fn ポート付きのurlも扱える() {
        assert_eq!(
            build_endpoint_url("http://localhost:8080").unwrap(),
            "http://localhost:8080/api/mcp"
        );
    }

    #[test]
    fn スキームの無いurlは拒否する() {
        let err = build_endpoint_url("autopost.example.com").unwrap_err();

        assert!(err.to_string().contains("http:// or https://"));
    }

    #[test]
    fn 空のurlは拒否する() {
        assert!(build_endpoint_url("").is_err());
        assert!(build_endpoint_url("   ").is_err());
    }

    // --- キーの解決 ---

    #[test]
    fn 専用キーがあればそれを使う() {
        let config = config_with(Some("web-secret"), Some("mcp-secret"));

        assert_eq!(resolve_api_key(&config).unwrap(), "mcp-secret");
    }

    #[test]
    fn 専用キーが無ければsecret_keyを使う() {
        let config = config_with(Some("web-secret"), None);

        assert_eq!(resolve_api_key(&config).unwrap(), "web-secret");
    }

    #[test]
    fn 空の専用キーは未設定として扱う() {
        let config = config_with(Some("web-secret"), Some(""));

        assert_eq!(resolve_api_key(&config).unwrap(), "web-secret");
    }

    #[test]
    fn キーがどこにも無ければ生成方法を案内する() {
        let config = config_with(None, None);
        let err = resolve_api_key(&config).unwrap_err();

        assert!(err.to_string().contains("No API key found"));
        assert!(err.to_string().contains("openssl rand -hex 32"));
    }

    // --- claude のコマンド組み立て ---

    #[test]
    fn claude_addの引数を組み立てる() {
        let args = claude_add_args(
            "blog-autopost",
            "https://autopost.example.com/api/mcp",
            "my-key",
            "user",
        );

        assert_eq!(
            args,
            vec![
                "mcp",
                "add",
                "--transport",
                "http",
                "--scope",
                "user",
                "blog-autopost",
                "https://autopost.example.com/api/mcp",
                "--header",
                "X-Api-Key: my-key",
            ]
        );
    }

    #[test]
    fn claude_removeの引数を組み立てる() {
        let args = claude_remove_args("blog-autopost", "user");

        assert_eq!(
            args,
            vec!["mcp", "remove", "--scope", "user", "blog-autopost"]
        );
    }

    #[test]
    fn 空白を含む引数は引用する() {
        let args = vec!["add".to_string(), "X-Api-Key: my-key".to_string()];

        assert_eq!(
            format_command("claude", &args),
            "claude add 'X-Api-Key: my-key'"
        );
    }

    // --- Claude 向けの JSON ---

    #[test]
    fn claudeのjson断片はtypeを含む() {
        let snippet = claude_json_snippet(
            "blog-autopost",
            "https://autopost.example.com/api/mcp",
            "my-key",
        );
        let value: serde_json::Value = serde_json::from_str(&snippet).unwrap();

        let entry = &value["mcpServers"]["blog-autopost"];
        // type が無いと stdio サーバと解釈されて動かない
        assert_eq!(entry["type"], "http");
        assert_eq!(entry["url"], "https://autopost.example.com/api/mcp");
        assert_eq!(entry["headers"]["X-Api-Key"], "my-key");
    }

    // --- Codex の TOML 編集 ---

    #[test]
    fn 空の設定へ定義を追記できる() {
        let out = upsert_codex_entry(
            "",
            "blog-autopost",
            "https://autopost.example.com/api/mcp",
            "BLOG_AUTOPOST_MCP_KEY",
        )
        .unwrap();

        assert!(
            out.contains("[mcp_servers.blog-autopost]"),
            "実際の内容:\n{}",
            out
        );
        assert!(out.contains(r#"url = "https://autopost.example.com/api/mcp""#));
        assert!(out.contains(r#"bearer_token_env_var = "BLOG_AUTOPOST_MCP_KEY""#));
    }

    #[test]
    fn キーは設定ファイルへ書かない() {
        let out = upsert_codex_entry(
            "",
            "blog-autopost",
            "https://autopost.example.com/api/mcp",
            "BLOG_AUTOPOST_MCP_KEY",
        )
        .unwrap();

        // Codex は bearer_token を受け付けないうえ、平文で残したくない
        assert!(!out.contains("bearer_token ="));
    }

    #[test]
    fn 既存のコメントと設定を保つ() {
        let existing = r#"# my codex settings
model = "gpt-5"

[features]
experimental_use_rmcp_client = true

[mcp_servers.other]
command = "npx"
args = ["some-server"]
"#;

        let out = upsert_codex_entry(
            existing,
            "blog-autopost",
            "https://autopost.example.com/api/mcp",
            "BLOG_AUTOPOST_MCP_KEY",
        )
        .unwrap();

        assert!(
            out.contains("# my codex settings"),
            "コメントが失われた:\n{}",
            out
        );
        assert!(out.contains(r#"model = "gpt-5""#));
        assert!(out.contains("experimental_use_rmcp_client = true"));
        assert!(
            out.contains("[mcp_servers.other]"),
            "他の定義が消えた:\n{}",
            out
        );
        assert!(out.contains("[mcp_servers.blog-autopost]"));
    }

    #[test]
    fn 同じ名前の定義は置き換える() {
        let existing = r#"[mcp_servers.blog-autopost]
url = "https://old.example.com/api/mcp"
bearer_token_env_var = "OLD_KEY"
"#;

        let out = upsert_codex_entry(
            existing,
            "blog-autopost",
            "https://new.example.com/api/mcp",
            "NEW_KEY",
        )
        .unwrap();

        assert!(out.contains("https://new.example.com/api/mcp"));
        assert!(
            !out.contains("old.example.com"),
            "古い値が残っている:\n{}",
            out
        );
        assert!(!out.contains("OLD_KEY"));
    }

    #[test]
    fn 壊れたtomlはエラーになる() {
        let err = upsert_codex_entry(
            "this is [not valid",
            "n",
            "https://e.example.com/api/mcp",
            "K",
        )
        .unwrap_err();

        assert!(err.to_string().contains("Failed to parse"));
    }

    #[test]
    fn 追記した設定は読み直せる() {
        let out = upsert_codex_entry(
            "",
            "blog-autopost",
            "https://autopost.example.com/api/mcp",
            "BLOG_AUTOPOST_MCP_KEY",
        )
        .unwrap();

        let parsed: toml_edit::DocumentMut = out.parse().expect("読み直せるはず");
        let entry = &parsed["mcp_servers"]["blog-autopost"];
        assert_eq!(
            entry["url"].as_str(),
            Some("https://autopost.example.com/api/mcp")
        );
    }

    // --- Codex の定義の削除 ---

    #[test]
    fn 定義を取り除ける() {
        let existing = r#"[mcp_servers.blog-autopost]
url = "https://autopost.example.com/api/mcp"
bearer_token_env_var = "BLOG_AUTOPOST_MCP_KEY"
"#;

        let (out, removed) = remove_codex_entry(existing, "blog-autopost").unwrap();

        assert!(removed);
        assert!(!out.contains("blog-autopost"), "実際の内容:\n{}", out);
        // 空になった節は残さない
        assert!(!out.contains("[mcp_servers]"));
    }

    #[test]
    fn 他の定義には触れない() {
        let existing = r#"# keep me
[mcp_servers.other]
command = "npx"

[mcp_servers.blog-autopost]
url = "https://autopost.example.com/api/mcp"
"#;

        let (out, removed) = remove_codex_entry(existing, "blog-autopost").unwrap();

        assert!(removed);
        assert!(out.contains("[mcp_servers.other]"), "実際の内容:\n{}", out);
        assert!(out.contains("# keep me"));
        assert!(!out.contains("blog-autopost"));
    }

    #[test]
    fn 存在しない定義の削除は何もしない() {
        let existing = r#"[mcp_servers.other]
command = "npx"
"#;

        let (out, removed) = remove_codex_entry(existing, "blog-autopost").unwrap();

        assert!(!removed);
        assert_eq!(out, existing);
    }

    #[test]
    fn mcp_serversが無い設定の削除も安全() {
        let existing = "model = \"gpt-5\"\n";

        let (out, removed) = remove_codex_entry(existing, "blog-autopost").unwrap();

        assert!(!removed);
        assert_eq!(out, existing);
    }

    #[test]
    fn 追記して削除すると元に戻る() {
        let original = "model = \"gpt-5\"\n";

        let added = upsert_codex_entry(
            original,
            "blog-autopost",
            "https://autopost.example.com/api/mcp",
            "K",
        )
        .unwrap();
        let (removed, did_remove) = remove_codex_entry(&added, "blog-autopost").unwrap();

        assert!(did_remove);
        assert_eq!(removed, original);
    }
}
