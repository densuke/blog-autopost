#!/usr/bin/env python3
import sqlite3
import json
import shutil
import argparse
from pathlib import Path

def migrate(python_dir: Path, rust_dir: Path):
    print(f"Starting migration from {python_dir} to {rust_dir}...")
    
    # ディレクトリ準備
    rust_data_dir = rust_dir / "data"
    rust_upload_dir = rust_data_dir / "uploads"
    rust_data_dir.mkdir(parents=True, exist_ok=True)
    rust_upload_dir.mkdir(parents=True, exist_ok=True)

    # 1. 既読記事のインポート
    python_data_dir = python_dir / "data"
    if not python_data_dir.exists():
        print(f"Error: Python data directory not found at {python_data_dir}")
        return

    articles_imported = []
    # articles_*.json または articles.json を探す
    for json_file in python_data_dir.glob("articles*.json"):
        # フィード名を抽出 (例: articles_main.json -> main, articles.json -> default)
        name = json_file.stem
        if name == "articles":
            feed_name = "default"
        elif name.startswith("articles_"):
            feed_name = name[len("articles_"):]
        else:
            feed_name = name
            
        try:
            with open(json_file, 'r', encoding='utf-8') as f:
                data = json.load(f)
                if isinstance(data, list):
                    for art in data:
                        articles_imported.append({
                            "title": art.get("title", ""),
                            "link": art.get("link", ""),
                            "published_parsed": art.get("published_parsed", ""),
                            "image_url": art.get("image_url"),
                            "feed_name": feed_name
                        })
            print(f"Loaded {len(data)} articles from {json_file.name} (feed: {feed_name})")
        except Exception as e:
            print(f"Warning: Failed to load {json_file.name}: {e}")

    # 重複排除（リンク基準）
    unique_articles = {}
    for art in articles_imported:
        unique_articles[art["link"]] = art
    
    rust_articles_file = rust_data_dir / "articles.json"
    with open(rust_articles_file, 'w', encoding='utf-8') as f:
        json.dump(list(unique_articles.values()), f, indent=2, ensure_ascii=False)
    print(f"Saved {len(unique_articles)} unique articles to {rust_articles_file}")

    # 2. 予約投稿のインポート
    db_path = python_data_dir / "scheduled_posts.db"
    if not db_path.exists():
        print(f"Warning: SQLite database not found at {db_path}. Skipping scheduled posts migration.")
        return

    try:
        conn = sqlite3.connect(db_path)
        cursor = conn.cursor()
        cursor.execute("SELECT id, content, scheduled_at, media_files, target_sns, status, error_message, created_at, updated_at FROM scheduled_posts")
        rows = cursor.fetchall()
        
        scheduled_posts = []
        for row in rows:
            post_id, content, scheduled_at, media_files_raw, target_sns_raw, status, error_message, created_at, updated_at = row
            
            # JSON文字列のパース
            media_files = []
            if media_files_raw:
                try:
                    media_files = json.loads(media_files_raw)
                except Exception:
                    media_files = []
                    
            target_sns = []
            if target_sns_raw:
                try:
                    target_sns = json.loads(target_sns_raw)
                except Exception:
                    target_sns = []

            # 画像ファイルの退避＆パス書き換え
            processed_media = []
            for media_path_str in media_files:
                media_path = Path(media_path_str)
                # Python版プロジェクト相対パス、または絶対パス
                src_path = python_dir / media_path_str if not media_path.is_absolute() else media_path
                if src_path.exists():
                    dest_file_name = src_path.name
                    dest_path = rust_upload_dir / dest_file_name
                    shutil.copy2(src_path, dest_path)
                    processed_media.append(f"data/uploads/{dest_file_name}")
                else:
                    processed_media.append(media_path_str)

            # Rustモデルに適合する形式へマッピング
            scheduled_posts.append({
                "id": post_id,
                "content": content,
                "scheduled_at": scheduled_at,
                "media_files": processed_media,
                "target_sns": target_sns,
                "link_url": None,
                "status": status,
                "error_message": error_message,
                "created_at": created_at,
                "updated_at": updated_at
            })
            
        conn.close()

        rust_posts_file = rust_data_dir / "scheduled_posts.json"
        with open(rust_posts_file, 'w', encoding='utf-8') as f:
            json.dump(scheduled_posts, f, indent=2, ensure_ascii=False)
        print(f"Imported {len(scheduled_posts)} scheduled posts to {rust_posts_file}")
        
    except Exception as e:
        print(f"Error during scheduled posts migration: {e}")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Migrate blog-autopost data from Python to Rust")
    parser.add_argument("--python-dir", type=str, default="../blog-autopost", help="Path to Python project directory")
    parser.add_argument("--rust-dir", type=str, default=".", help="Path to Rust project directory")
    args = parser.parse_args()
    
    migrate(Path(args.python_dir), Path(args.rust_dir))
