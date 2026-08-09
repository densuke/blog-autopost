//! ログインセッションをファイルへ永続化する。
//!
//! セッションはメモリ上の `HashMap` が正で、このストアはその写しを保存する。
//! デーモンを再起動しても、期限内のセッションはログインしたまま戻せる。
//!
//! 保存に失敗しても認証そのものは動き続ける必要があるため、
//! 呼び出し側はエラーをログに残して処理を続ける方針を取る。

use crate::web::session::Session;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// 一時ファイル名に使う連番。
///
/// 同じプロセス内で保存が重なっても、別々の一時ファイルになるようにする。
static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// セッションを JSON ファイルへ保存するストア。
pub struct JsonSessionStore {
    file_path: PathBuf,
    /// 保存の直列化に使う。書き出しの重なりを防ぐ。
    lock: Mutex<()>,
}

impl JsonSessionStore {
    /// 保存先のパスを指定してストアを作る。
    pub fn new<P: Into<PathBuf>>(file_path: P) -> Self {
        Self {
            file_path: file_path.into(),
            lock: Mutex::new(()),
        }
    }

    /// 保存先のパスを返す。
    pub fn path(&self) -> &Path {
        &self.file_path
    }

    /// セッションの一覧をファイルへ書き出す。
    ///
    /// セッションIDそのものが認証情報になるため、ファイルは所有者のみが
    /// 読み書きできる権限(0600)にする。
    ///
    /// ログインと期限の延長は同時に起こりうる。同じプロセス内では
    /// ロックで直列化し、さらに一時ファイル名を呼び出しごとに変えることで、
    /// 別プロセスが同じファイルを触っても書き込みが混ざらないようにする。
    pub fn save(&self, sessions: &HashMap<String, Session>) -> Result<()> {
        // ロックが毒されていても保存は続けたい。前の書き出しが途中で
        // 落ちただけで、これから書く内容には影響しない
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());

        if let Some(parent) = self.file_path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "Failed to create directory for sessions file: {}",
                    parent.display()
                )
            })?;
        }

        let content = serde_json::to_string_pretty(sessions)
            .context("Failed to serialize sessions to JSON")?;

        // 一時ファイルへ書いてから差し替える。書き込み中に落ちても
        // 既存のファイルが半端な内容で壊れない。
        let tmp_path = self.tmp_path();
        write_private(&tmp_path, content.as_bytes())?;
        if let Err(e) = std::fs::rename(&tmp_path, &self.file_path) {
            // 差し替えに失敗した一時ファイルは残しても意味がない
            std::fs::remove_file(&tmp_path).ok();
            return Err(e).context("Failed to replace sessions file");
        }

        Ok(())
    }

    /// 書き出しに使う一時ファイルのパスを組み立てる。
    ///
    /// プロセスIDと連番を混ぜ、他のプロセスや同時実行と衝突しないようにする。
    fn tmp_path(&self) -> PathBuf {
        let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let suffix = format!("tmp.{}.{}", std::process::id(), seq);
        self.file_path.with_extension(suffix)
    }

    /// ファイルからセッションを読み込む。
    ///
    /// 期限切れのセッションは読み込んだ時点で捨てる。ファイルが無い場合と
    /// 中身が壊れている場合はどちらも空として扱う。壊れたファイルで
    /// 起動できなくなるより、全員ログインし直すほうが害が小さい。
    pub fn load(&self, now: DateTime<Utc>) -> HashMap<String, Session> {
        let Ok(content) = std::fs::read_to_string(&self.file_path) else {
            return HashMap::new();
        };
        if content.trim().is_empty() {
            return HashMap::new();
        }

        match serde_json::from_str::<HashMap<String, Session>>(&content) {
            Ok(mut sessions) => {
                sessions.retain(|_, s| !s.is_expired(now));
                sessions
            }
            Err(e) => {
                println!("Failed to parse sessions file (starting empty): {e}");
                HashMap::new()
            }
        }
    }
}

/// 所有者だけが読み書きできる権限でファイルを書き出す。
///
/// 作成時から 0600 にするのが要点で、一瞬でも他者に読める状態を作らない。
/// 前回の残骸が別の権限で残っている場合に備え、書き出したあとに
/// 権限を明示し直す。Unix 以外では権限の考え方が異なるため何もしない。
#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .context("Failed to open sessions file for writing")?;
    file.write_all(bytes)
        .context("Failed to write sessions file")?;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .context("Failed to restrict permissions on sessions file")
}

#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    std::fs::write(path, bytes).context("Failed to write sessions file")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn store_in_temp() -> (tempfile::TempDir, JsonSessionStore) {
        let dir = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let store = JsonSessionStore::new(dir.path().join("sessions.json"));
        (dir, store)
    }

    #[test]
    fn 保存したセッションを読み戻せる() {
        let (_dir, store) = store_in_temp();
        let now = Utc::now();

        let mut sessions = HashMap::new();
        sessions.insert(
            "abc123".to_string(),
            Session::with_created_at("admin".to_string(), 24, now),
        );

        store.save(&sessions).expect("保存できる");
        let loaded = store.load(now);

        assert_eq!(loaded.len(), 1);
        let restored = loaded.get("abc123").expect("同じIDで引ける");
        assert_eq!(restored.username, "admin");
        assert_eq!(restored.expires_at, now + Duration::hours(24));
    }

    #[test]
    fn 読み込み時に期限切れのセッションを捨てる() {
        let (_dir, store) = store_in_temp();
        let now = Utc::now();

        let mut sessions = HashMap::new();
        sessions.insert(
            "live".to_string(),
            Session::with_created_at("admin".to_string(), 24, now),
        );
        sessions.insert(
            "dead".to_string(),
            Session::with_created_at("admin".to_string(), 1, now - Duration::hours(2)),
        );

        store.save(&sessions).expect("保存できる");
        let loaded = store.load(now);

        assert_eq!(loaded.len(), 1);
        assert!(loaded.contains_key("live"));
        assert!(!loaded.contains_key("dead"));
    }

    #[test]
    fn ファイルが無ければ空として扱う() {
        let (_dir, store) = store_in_temp();

        let loaded = store.load(Utc::now());

        assert!(loaded.is_empty());
    }

    #[test]
    fn 壊れたファイルは空として扱う() {
        let (_dir, store) = store_in_temp();
        std::fs::write(store.path(), "{ this is not json").expect("書き込める");

        let loaded = store.load(Utc::now());

        assert!(loaded.is_empty());
    }

    #[test]
    fn 空のファイルは空として扱う() {
        let (_dir, store) = store_in_temp();
        std::fs::write(store.path(), "   \n").expect("書き込める");

        let loaded = store.load(Utc::now());

        assert!(loaded.is_empty());
    }

    #[test]
    fn 保存すると親ディレクトリを作る() {
        let dir = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let store = JsonSessionStore::new(dir.path().join("nested").join("sessions.json"));

        store.save(&HashMap::new()).expect("保存できる");

        assert!(store.path().exists());
    }

    #[test]
    fn 上書き保存すると前の内容が残らない() {
        let (_dir, store) = store_in_temp();
        let now = Utc::now();

        let mut first = HashMap::new();
        first.insert(
            "old".to_string(),
            Session::with_created_at("admin".to_string(), 24, now),
        );
        store.save(&first).expect("保存できる");

        let mut second = HashMap::new();
        second.insert(
            "new".to_string(),
            Session::with_created_at("admin".to_string(), 24, now),
        );
        store.save(&second).expect("保存できる");

        let loaded = store.load(now);
        assert_eq!(loaded.len(), 1);
        assert!(loaded.contains_key("new"));
    }

    #[test]
    fn ディレクトリを作れないときはエラーを返す() {
        let dir = tempfile::tempdir().expect("一時ディレクトリを作れる");
        // 親になるはずの場所をファイルで塞ぐ
        let blocker = dir.path().join("blocked");
        std::fs::write(&blocker, "not a directory").expect("書き込める");
        let store = JsonSessionStore::new(blocker.join("sessions.json"));

        let result = store.save(&HashMap::new());

        // 握りつぶさず、原因の分かるエラーとして返す
        let err = result.expect_err("保存は失敗する");
        assert!(
            format!("{err:#}").contains("Failed to create directory"),
            "実際のエラー: {err:#}"
        );
    }

    #[test]
    fn 一時ファイルの名前は呼ぶたびに変わる() {
        let (_dir, store) = store_in_temp();

        let first = store.tmp_path();
        let second = store.tmp_path();

        // 同じ名前だと保存が重なったときに書き込みが混ざる
        assert_ne!(first, second);
    }

    #[test]
    fn 並行して保存してもファイルが壊れない() {
        let dir = tempfile::tempdir().expect("一時ディレクトリを作れる");
        let store = std::sync::Arc::new(JsonSessionStore::new(dir.path().join("sessions.json")));
        let now = Utc::now();

        let handles: Vec<_> = (0..8)
            .map(|i| {
                let store = store.clone();
                std::thread::spawn(move || {
                    let mut sessions = HashMap::new();
                    sessions.insert(
                        format!("session-{i}"),
                        Session::with_created_at("admin".to_string(), 24, now),
                    );
                    store.save(&sessions).expect("保存できる");
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("スレッドが完了する");
        }

        // どのスレッドの内容が最後に残るかは決まらないが、
        // 読み戻せる正しい JSON になっていること
        let loaded = store.load(now);
        assert_eq!(loaded.len(), 1);

        // 一時ファイルが残っていないこと
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .expect("読める")
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains("tmp"))
            .collect();
        assert!(leftovers.is_empty(), "残った一時ファイル: {leftovers:?}");
    }

    #[cfg(unix)]
    #[test]
    fn 保存したファイルは所有者だけが読み書きできる() {
        use std::os::unix::fs::PermissionsExt;

        let (_dir, store) = store_in_temp();
        store.save(&HashMap::new()).expect("保存できる");

        let mode = std::fs::metadata(store.path())
            .expect("メタデータを取れる")
            .permissions()
            .mode();

        // 下位9ビットだけを見る。ファイル種別のビットは比較対象にしない
        assert_eq!(mode & 0o777, 0o600);
    }
}
