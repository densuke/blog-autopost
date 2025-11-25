#!/usr/bin/env python
"""JSON から SQLite へのマイグレーションスクリプト

既存の data/articles.json または scheduled_posts.json を SQLite に移行します。
このスクリプトは以下の処理を実行：
1. 既存JSONファイルを読み込み
2. SQLiteデータベースを初期化
3. JSONデータをSQLiteに挿入
4. 整合性チェック

使用法:
    python scripts/migrate_json_to_sqlite.py [--json-path path/to/file.json] [--db-path path/to/db.db]
"""

import json
import argparse
import sys
from pathlib import Path
from datetime import datetime

# プロジェクトルートをPythonパスに追加
sys.path.insert(0, str(Path(__file__).parent.parent))

from src.web.models import init_db, get_session, ScheduledPostDB


def load_json_posts(json_path: Path) -> list:
    """JSONファイルから投稿データを読み込み"""
    if not json_path.exists():
        print(f"警告: JSONファイルが見つかりません: {json_path}")
        return []
    
    try:
        with open(json_path, 'r', encoding='utf-8') as f:
            data = json.load(f)
        print(f"✅ JSONファイルを読み込み: {json_path} ({len(data)} 件)")
        return data if isinstance(data, list) else []
    except Exception as e:
        print(f"❌ JSON読み込みエラー: {e}")
        return []


def migrate_posts_to_db(json_posts: list, db_path: str) -> tuple[int, int]:
    """JSONの投稿データをSQLiteに移行
    
    Returns:
        (成功件数, 失敗件数)
    """
    engine = init_db(db_path)
    session = get_session(engine)
    
    success_count = 0
    fail_count = 0
    
    print(f"📝 {len(json_posts)} 件のデータをSQLiteに移行中...")
    
    for i, post_data in enumerate(json_posts, 1):
        try:
            # JSON形式からScheduledPostDBを構築
            post = ScheduledPostDB.from_dict(post_data)
            
            # データベースに追加
            session.add(post)
            success_count += 1
            
            if i % 100 == 0:
                print(f"  処理中: {i}/{len(json_posts)}")
        
        except Exception as e:
            print(f"  ⚠️  行 {i}: 移行失敗 - {e}")
            fail_count += 1
    
    # コミット
    try:
        session.commit()
        print("✅ コミット完了")
    except Exception as e:
        session.rollback()
        print(f"❌ コミット失敗: {e}")
        return 0, len(json_posts)
    finally:
        session.close()
    
    return success_count, fail_count


def verify_migration(json_posts: list, db_path: str) -> bool:
    """マイグレーション成功を検証"""
    session = get_session(__import__('src.web.models', fromlist=['get_db_engine']).get_db_engine(db_path))
    
    # データベース内の件数を確認
    db_count = session.query(ScheduledPostDB).count()
    json_count = len(json_posts)
    
    print("\n📊 検証結果:")
    print(f"  JSONデータ件数: {json_count}")
    print(f"  SQLiteデータ件数: {db_count}")
    
    if db_count == json_count:
        print("✅ マイグレーション成功: すべてのデータが正常に移行されました")
        session.close()
        return True
    else:
        print(f"⚠️  マイグレーション部分成功: {db_count}/{json_count} 件のみ移行")
        session.close()
        return db_count > 0


def backup_json(json_path: Path) -> Path:
    """JSONファイルをバックアップ"""
    if not json_path.exists():
        return None
    
    backup_path = json_path.with_stem(f"{json_path.stem}_backup_{datetime.now().strftime('%Y%m%d_%H%M%S')}")
    
    try:
        import shutil
        shutil.copy(json_path, backup_path)
        print(f"💾 バックアップ作成: {backup_path}")
        return backup_path
    except Exception as e:
        print(f"⚠️  バックアップ作成失敗: {e}")
        return None


def main():
    """メイン処理"""
    parser = argparse.ArgumentParser(
        description='JSON から SQLite へのマイグレーション'
    )
    parser.add_argument(
        '--json-path',
        type=Path,
        default=Path('data/scheduled_posts.json'),
        help='JSONファイルパス（デフォルト: data/scheduled_posts.json）'
    )
    parser.add_argument(
        '--db-path',
        type=str,
        default='data/scheduled_posts.db',
        help='SQLiteデータベースパス（デフォルト: data/scheduled_posts.db）'
    )
    parser.add_argument(
        '--no-backup',
        action='store_true',
        help='バックアップを作成しない（推奨: 常にバックアップを作成してください）'
    )
    parser.add_argument(
        '--verify-only',
        action='store_true',
        help='検証のみ実行（実際には移行しない）'
    )
    
    args = parser.parse_args()
    
    print("=" * 60)
    print("JSON → SQLite マイグレーションスクリプト")
    print("=" * 60)
    
    # JSONファイルが複数存在する可能性をチェック
    json_candidates = [
        Path('data/scheduled_posts.json'),
        Path('data/articles.json'),
        args.json_path,
    ]
    
    json_path = None
    for candidate in json_candidates:
        if candidate.exists():
            json_path = candidate
            break
    
    if not json_path:
        print("❌ JSONファイルが見つかりません")
        print(f"確認パス: {json_candidates}")
        return 1
    
    print(f"\n📂 JSONファイル: {json_path}")
    print(f"📁 SQLiteデータベース: {args.db_path}")
    
    # JSONデータを読み込み
    json_posts = load_json_posts(json_path)
    
    if not json_posts:
        print("⚠️  移行するデータがありません")
        return 0
    
    # バックアップ
    if not args.no_backup:
        backup_json(json_path)
    
    # 検証のみモード
    if args.verify_only:
        print("\n🔍 検証のみモード（実際には移行しません）")
        engine = init_db(args.db_path)
        verify_migration(json_posts, args.db_path)
        return 0
    
    # マイグレーション実行
    success, fail = migrate_posts_to_db(json_posts, args.db_path)
    
    print(f"\n📈 移行結果: {success} 成功 / {fail} 失敗")
    
    # 検証
    if verify_migration(json_posts, args.db_path):
        print("\n✨ マイグレーション完了!")
        return 0
    else:
        print("\n⚠️  マイグレーションに問題があります")
        return 1


if __name__ == '__main__':
    exit(main())
