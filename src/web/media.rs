//! 添付メディアのパス検証と保存。
//!
//! MCP の tool はローカルのファイルパスを引数に取るため、検証なしでは
//! 認証済みクライアントが任意のファイルを SNS へ送信できてしまう。
//! 許可ディレクトリの内側に限定し、形式とサイズも確かめる。

use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};

/// 添付できるファイルサイズの上限 (10MB)。
///
/// Web UI のアップロードと同じ値にしてある。
pub const MAX_MEDIA_BYTES: u64 = 10 * 1024 * 1024;

/// 許可ディレクトリが設定されていないときに使う既定値。
///
/// Web UI からアップロードしたファイルは `data/uploads` に入るため、
/// 通常の使い方はこれで足りる。
pub const DEFAULT_ALLOWED_DIRS: [&str; 2] = ["data/uploads", "data"];

/// 設定値から許可ディレクトリの一覧を組み立てる。
pub fn resolve_allowed_dirs(configured: Option<&[String]>) -> Vec<PathBuf> {
    match configured {
        Some(dirs) if !dirs.is_empty() => dirs.iter().map(PathBuf::from).collect(),
        _ => DEFAULT_ALLOWED_DIRS.iter().map(PathBuf::from).collect(),
    }
}

/// バイト列が対応する画像または動画かどうかを判定する。
///
/// 拡張子ではなく中身で判断するため、名前を偽っても通らない。
pub fn is_supported_media(bytes: &[u8]) -> bool {
    crate::sns::is_supported_image(bytes) || is_supported_video(bytes)
}

/// バイト列が対応する動画かどうかを判定する。
///
/// MP4 と QuickTime はどちらも ISO Base Media 形式で、
/// 先頭ボックスのサイズ4バイトに続いて `ftyp` が現れる。
fn is_supported_video(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && &bytes[4..8] == b"ftyp"
}

/// MCP から渡されたメディアパスを検証し、正規化された絶対パスを返す。
///
/// 許可ディレクトリ配下にあること、対応する画像/動画形式であること、
/// サイズ上限内であることを確認する。シンボリックリンクは解決してから
/// 判定するため、リンクで外へ抜けることはできない。
pub fn validate_media_path(
    allowed_dirs: &[PathBuf],
    input: &str,
    max_bytes: u64,
) -> Result<PathBuf> {
    let path = Path::new(input);

    // canonicalize がシンボリックリンクと `..` の両方を解決する
    let resolved = path
        .canonicalize()
        .with_context(|| format!("Media file not found: {}", input))?;

    let metadata = std::fs::metadata(&resolved)
        .with_context(|| format!("Failed to read media metadata: {}", input))?;

    if !metadata.is_file() {
        return Err(anyhow!("Media path is not a regular file: {}", input));
    }

    if !is_within_allowed_dirs(allowed_dirs, &resolved) {
        return Err(anyhow!(
            "Media path is outside the allowed directories: {}. \
             Add its directory to mcp.allowed_media_dirs if this is intended.",
            input
        ));
    }

    if metadata.len() > max_bytes {
        return Err(anyhow!(
            "Media file is too large: {} bytes (limit {} bytes)",
            metadata.len(),
            max_bytes
        ));
    }

    // 形式判定には先頭だけあれば足りるが、画像のデコーダに合わせて全体を読む
    let bytes = std::fs::read(&resolved)
        .with_context(|| format!("Failed to read media file: {}", input))?;
    if !is_supported_media(&bytes) {
        return Err(anyhow!("Unsupported media format: {}", input));
    }

    Ok(resolved)
}

/// 解決済みのパスが許可ディレクトリのいずれかの配下にあるかを判定する。
///
/// 許可ディレクトリ側も解決してから比較する。解決できないディレクトリは
/// 存在しないものとして扱い、判定に使わない。
fn is_within_allowed_dirs(allowed_dirs: &[PathBuf], resolved: &Path) -> bool {
    allowed_dirs.iter().any(|dir| {
        dir.canonicalize()
            .map(|allowed| resolved.starts_with(&allowed))
            .unwrap_or(false)
    })
}

/// ファイル名から保存に使える安全な名前を作る。
///
/// 英数字とごく一部の記号だけを残すため、パス区切りや `..` は残らない。
pub fn sanitize_file_name(file_name: &str) -> String {
    file_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// 重複しない保存先ファイル名を作る。
pub fn unique_file_name(file_name: &str) -> String {
    let timestamp = chrono::Utc::now().timestamp_micros();
    format!("{}_{}", timestamp, sanitize_file_name(file_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// 1x1 の PNG。画像として認識される最小限のデータ。
    fn png_bytes() -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        image::RgbImage::new(1, 1)
            .write_to(&mut buf, image::ImageFormat::Png)
            .expect("PNGの書き出しに失敗");
        buf.into_inner()
    }

    /// ISO Base Media 形式の先頭だけを模したデータ。
    fn mp4_header() -> Vec<u8> {
        let mut bytes = vec![0x00, 0x00, 0x00, 0x18];
        bytes.extend_from_slice(b"ftypisom");
        bytes.extend_from_slice(b"\x00\x00\x02\x00");
        bytes
    }

    /// 許可ディレクトリ1つを持つ一時領域を作る。
    fn setup() -> (tempfile::TempDir, PathBuf, Vec<PathBuf>) {
        let dir = tempfile::TempDir::new().expect("一時ディレクトリの作成に失敗");
        let allowed = dir.path().join("uploads");
        std::fs::create_dir_all(&allowed).expect("許可ディレクトリの作成に失敗");
        let dirs = vec![allowed.clone()];
        (dir, allowed, dirs)
    }

    /// 指定した内容のファイルを作る。
    fn write_file(path: &Path, content: &[u8]) {
        let mut f = std::fs::File::create(path).expect("ファイルの作成に失敗");
        f.write_all(content).expect("書き込みに失敗");
    }

    // --- resolve_allowed_dirs ---

    #[test]
    fn 未設定なら既定の許可ディレクトリを使う() {
        assert_eq!(
            resolve_allowed_dirs(None),
            vec![PathBuf::from("data/uploads"), PathBuf::from("data")]
        );
    }

    #[test]
    fn 空リストは未設定として扱う() {
        assert_eq!(resolve_allowed_dirs(Some(&[])), resolve_allowed_dirs(None));
    }

    #[test]
    fn 設定があればそれを使う() {
        let configured = vec!["/srv/media".to_string()];
        assert_eq!(
            resolve_allowed_dirs(Some(&configured)),
            vec![PathBuf::from("/srv/media")]
        );
    }

    // --- 形式判定 ---

    #[test]
    fn pngは対応形式として扱う() {
        assert!(is_supported_media(&png_bytes()));
    }

    #[test]
    fn mp4のftypボックスを動画として扱う() {
        assert!(is_supported_media(&mp4_header()));
    }

    #[test]
    fn テキストは対応形式ではない() {
        assert!(!is_supported_media(b"this is not media"));
        assert!(!is_supported_media(b""));
    }

    // --- validate_media_path ---

    #[test]
    fn 許可ディレクトリ内の画像は通る() {
        let (_dir, allowed, dirs) = setup();
        let file = allowed.join("ok.png");
        write_file(&file, &png_bytes());

        let resolved = validate_media_path(&dirs, file.to_str().unwrap(), MAX_MEDIA_BYTES).unwrap();

        assert_eq!(resolved, file.canonicalize().unwrap());
    }

    #[test]
    fn 許可ディレクトリの外は拒否する() {
        let (dir, _allowed, dirs) = setup();
        let outside = dir.path().join("secret.png");
        write_file(&outside, &png_bytes());

        let err =
            validate_media_path(&dirs, outside.to_str().unwrap(), MAX_MEDIA_BYTES).unwrap_err();

        assert!(
            err.to_string().contains("outside the allowed directories"),
            "実際のエラー: {}",
            err
        );
    }

    #[test]
    fn 親ディレクトリを辿る指定は拒否する() {
        let (dir, allowed, dirs) = setup();
        let outside = dir.path().join("secret.png");
        write_file(&outside, &png_bytes());

        // uploads/../secret.png は canonicalize で許可ディレクトリの外へ出る
        let traversal = allowed.join("..").join("secret.png");
        let err =
            validate_media_path(&dirs, traversal.to_str().unwrap(), MAX_MEDIA_BYTES).unwrap_err();

        assert!(err.to_string().contains("outside the allowed directories"));
    }

    #[cfg(unix)]
    #[test]
    fn 外へ抜けるシンボリックリンクは拒否する() {
        let (dir, allowed, dirs) = setup();
        let outside = dir.path().join("secret.png");
        write_file(&outside, &png_bytes());

        let link = allowed.join("link.png");
        std::os::unix::fs::symlink(&outside, &link).expect("シンボリックリンクの作成に失敗");

        // リンク自体は許可ディレクトリの中にあるが、解決先は外
        let err = validate_media_path(&dirs, link.to_str().unwrap(), MAX_MEDIA_BYTES).unwrap_err();

        assert!(err.to_string().contains("outside the allowed directories"));
    }

    #[test]
    fn 存在しないファイルは拒否する() {
        let (_dir, allowed, dirs) = setup();
        let missing = allowed.join("nope.png");

        let err =
            validate_media_path(&dirs, missing.to_str().unwrap(), MAX_MEDIA_BYTES).unwrap_err();

        assert!(
            err.to_string().contains("not found"),
            "実際のエラー: {}",
            err
        );
    }

    #[test]
    fn ディレクトリの指定は拒否する() {
        let (_dir, allowed, dirs) = setup();
        let sub = allowed.join("subdir");
        std::fs::create_dir_all(&sub).unwrap();

        let err = validate_media_path(&dirs, sub.to_str().unwrap(), MAX_MEDIA_BYTES).unwrap_err();

        assert!(err.to_string().contains("not a regular file"));
    }

    #[test]
    fn 対応していない形式は拒否する() {
        let (_dir, allowed, dirs) = setup();
        let file = allowed.join("note.png");
        // 拡張子を偽っても中身で弾く
        write_file(&file, b"this is plain text");

        let err = validate_media_path(&dirs, file.to_str().unwrap(), MAX_MEDIA_BYTES).unwrap_err();

        assert!(err.to_string().contains("Unsupported media format"));
    }

    #[test]
    fn サイズ超過は拒否する() {
        let (_dir, allowed, dirs) = setup();
        let file = allowed.join("big.png");
        write_file(&file, &png_bytes());

        // 上限を実ファイルより小さくして超過させる
        let err = validate_media_path(&dirs, file.to_str().unwrap(), 1).unwrap_err();

        assert!(
            err.to_string().contains("too large"),
            "実際のエラー: {}",
            err
        );
    }

    #[test]
    fn 許可ディレクトリが存在しなければ何も通さない() {
        let (_dir, allowed, _) = setup();
        let file = allowed.join("ok.png");
        write_file(&file, &png_bytes());

        let dirs = vec![PathBuf::from("/no/such/dir")];
        let err = validate_media_path(&dirs, file.to_str().unwrap(), MAX_MEDIA_BYTES).unwrap_err();

        assert!(err.to_string().contains("outside the allowed directories"));
    }

    #[test]
    fn 複数の許可ディレクトリのいずれかに入っていれば通る() {
        let (dir, allowed, _) = setup();
        let another = dir.path().join("other");
        std::fs::create_dir_all(&another).unwrap();
        let file = another.join("ok.png");
        write_file(&file, &png_bytes());

        let dirs = vec![allowed, another];
        assert!(validate_media_path(&dirs, file.to_str().unwrap(), MAX_MEDIA_BYTES).is_ok());
    }

    // --- ファイル名 ---

    #[test]
    fn ファイル名から危険な文字を落とす() {
        assert_eq!(sanitize_file_name("a/b/../c.png"), "a_b_.._c.png");
        // 非ASCIIは1文字につき1つの '_' に置き換わる
        assert_eq!(sanitize_file_name("画像.png"), "__.png");
        assert_eq!(sanitize_file_name("ok-name_1.png"), "ok-name_1.png");
    }

    #[test]
    fn 保存名はタイムスタンプで一意になる() {
        let name = unique_file_name("photo.png");

        assert!(name.ends_with("_photo.png"), "実際の値: {}", name);
        assert!(!name.contains('/'));
    }
}
