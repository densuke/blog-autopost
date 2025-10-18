# Phase 4: プラグイン・Web API テスト拡充 + コード品質改善

## 現在のカバレッジ (全体 69%)

### 低カバレッジモジュール（優先度高）
1. bluesky.py: 43% (151行未カバー)
2. mastodon.py: 40% (49行未カバー)
3. misskey.py: 45% (46行未カバー)
4. x.py: 61% (13行未カバー)
5. dao.py: 48% (45行未カバー)
6. posting_service.py: 67% (21行未カバー)

### 目標
- Phase 4.1: bluesky.py → 70%以上
- Phase 4.2: mastodon.py → 70%以上
- Phase 4.3: misskey.py → 70%以上
- Phase 4.4: x.py → 75%以上
- Phase 4.5: Web API (dao.py, posting_service.py) → 70%以上

## コード品質改善 (Phase 5)
- ruff による静的解析
- mypy による型チェック
- docstring の補完
- エラー処理の統一化

## 実装順序
1. Phase 4 でテストカバレッジを 69% → 75%以上に改善
2. Phase 5 でコード品質改善を実施
3. 最終カバレッジ測定と検証
