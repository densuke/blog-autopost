# Design Document: unified-post-form-ui

## 1. アーキテクチャ設計

### 1.1 コンポーネント構成

```
┌─────────────────────────────────────────────────────┐
│         投稿フォームコンテナ                         │
├─────────────────────────────────────────────────────┤
│                                                     │
│  ┌─────────────────────────────────────────────┐  │
│  │  投稿内容入力エリア                         │  │
│  │  - テキスト入力                             │  │
│  │  - メディアアップロード                     │  │
│  │  - 対象SNS選択                              │  │
│  └─────────────────────────────────────────────┘  │
│                                                     │
│  ┌─────────────────────────────────────────────┐  │
│  │  投稿方法選択グループ（新規追加）           │  │
│  │  ○ すぐに投稿                                │  │
│  │  ○ 時間指定の予約投稿                        │  │
│  │  ○ 次のタイミングで投稿                      │  │
│  └─────────────────────────────────────────────┘  │
│                                                     │
│  ┌─────────────────────────────────────────────┐  │
│  │  条件付き表示エリア#1（時間指定用）         │  │
│  │  - 日時ピッカー（display: none|block）      │  │
│  │  - トレランス説明                           │  │
│  └─────────────────────────────────────────────┘  │
│                                                     │
│  ┌─────────────────────────────────────────────┐  │
│  │  条件付き表示エリア#2（次のタイミング用）   │  │
│  │  - スロット検索説明（display: none|block）  │  │
│  └─────────────────────────────────────────────┘  │
│                                                     │
│  ┌─────────────────────────────────────────────┐  │
│  │  統合投稿ボタン（ラベルは動的変更）         │  │
│  │  - 「投稿」または「予約投稿」またはその他   │  │
│  └─────────────────────────────────────────────┘  │
│                                                     │
└─────────────────────────────────────────────────────┘
```

### 1.2 データフロー

```
ユーザー入力
    ↓
ラジオボタン選択 → LocalStorageに保存
    ↓
UIコンポーネント更新（表示/非表示）
    ↓
ボタンクリック
    ↓
選択方法に応じたAPI呼び出し
    ├─ すぐに投稿 → POST /api/post
    ├─ 時間指定 → POST /api/scheduled-posts
    └─ 次のタイミング → POST /api/scheduled-posts/next
    ↓
レスポンス処理 → 予約一覧更新
```

### 1.3 状態管理

```javascript
// グローバル状態
{
  postMethod: 'immediate' | 'scheduled' | 'next_timing',  // 投稿方法
  postContent: string,                                     // 投稿内容
  scheduledAt: Date | null,                               // 予約日時
  targetSNS: string[],                                    // 対象SNS
  mediaFiles: File[]                                      // メディアファイル
}

// LocalStorage
post_method_preference: 'immediate' | 'scheduled' | 'next_timing'
```

---

## 2. UI/UX設計

### 2.1 ラジオボタングループ設計

**要素ID**: `post-method-group`
**レイアウト**: 縦積み（全ブレークポイント共通）

```html
<fieldset id="post-method-group">
  <legend>投稿方法を選択</legend>
  
  <ion-item lines="none">
    <ion-label>
      <input type="radio" name="post_method" value="immediate" />
      <span>すぐに投稿</span>
    </ion-label>
    <ion-note slot="end">各SNSに即座に投稿します</ion-note>
  </ion-item>
  
  <ion-item lines="none">
    <ion-label>
      <input type="radio" name="post_method" value="scheduled" />
      <span>時間指定の予約投稿</span>
    </ion-label>
    <ion-note slot="end">指定した日時に投稿します</ion-note>
  </ion-item>
  
  <ion-item lines="none">
    <ion-label>
      <input type="radio" name="post_method" value="next_timing" />
      <span>次のタイミングで投稿</span>
    </ion-label>
    <ion-note slot="end">各SNSの最適タイミングに自動予約</ion-note>
  </ion-item>
</fieldset>
```

**スタイル**:
- 各ラジオボタンに `ion-item` でパディング確保
- 選択時の視認性: `ion-item` の背景色変更
- アクセシビリティ: `<label>` と `<input type="radio">` を関連付け

### 2.2 条件付き表示エリア設計

#### エリア#1: 日時ピッカー（時間指定用）

**要素ID**: `scheduled-controls-area`
**初期状態**: `display: none`
**表示条件**: `postMethod === 'scheduled'`

```html
<div id="scheduled-controls-area" class="scheduled-controls" style="display: none;">
  <ion-item class="schedule-item" lines="none">
    <ion-label position="stacked">予約日時 (JST: UTC+9)</ion-label>
    <div class="schedule-picker-group">
      <ion-datetime-button datetime="schedule_time"></ion-datetime-button>
      <ion-modal id="datetime-modal" keepContentsMounted="true">
        <ion-datetime 
          id="schedule_time" 
          name="schedule_time"
          presentation="date-time"
          show-default-buttons="true"
          prefer-wheel="true">
        </ion-datetime>
      </ion-modal>
      <ion-input
        id="schedule-manual-input"
        type="datetime-local"
        inputmode="numeric"
        step="60"
        autocomplete="off"
        fill="outline"
        placeholder="YYYY-MM-DDTHH:mm">
      </ion-input>
    </div>
    <ion-note slot="helper" class="schedule-controls-helper">
      日本時間で入力してください。直接入力やカーソルキーで細かく調整できます。
    </ion-note>
  </ion-item>
</div>
```

#### エリア#2: スロット検索説明（次のタイミング用）

**要素ID**: `next-timing-info-area`
**初期状態**: `display: none`
**表示条件**: `postMethod === 'next_timing'`

```html
<div id="next-timing-info-area" style="display: none;">
  <ion-card class="info-card">
    <ion-card-content>
      <ion-text>
        <p>
          <ion-icon name="information-circle"></ion-icon>
          Slot Finder が選んだ次のタイミングで各SNSに予約投稿します。
        </p>
        <small>タイムゾーン: サーバーのローカルタイムゾーン / スロット検索範囲: 7日以内</small>
      </ion-text>
    </ion-card-content>
  </ion-card>
</div>
```

### 2.3 ボタン統一設計

**要素ID**: `post-button`
**初期ラベル**: 「投稿」（すぐに投稿選択時）
**動的ラベル変更**:
- 「すぐに投稿」選択: `textContent = '投稿'`
- 「時間指定」選択: `textContent = '予約投稿'`
- 「次のタイミング」選択: `textContent = '次のタイミングで予約'`

```html
<ion-button id="post-button" expand="block" type="submit">
  <ion-icon slot="start" name="send"></ion-icon>
  投稿
</ion-button>
```

### 2.4 レイアウト設計

#### モバイル（< 768px）

```
┌─────────────────┐
│ 投稿内容        │
│                 │
├─────────────────┤
│ 投稿方法選択    │
│ ○ すぐに        │
│ ○ 時間指定      │
│ ○ 次のタイミング│
├─────────────────┤
│ [条件付き表示]  │
│ (時間指定)      │
├─────────────────┤
│ [投稿ボタン]    │
└─────────────────┘
```

**CSS**:
- `ion-content` の padding: 12px（既存）
- ラジオボタングループ: 100% 幅
- ボタン: 100% 幅（`expand="block"`）

#### タブレット（768px - 1024px）

```
┌──────────────────────────────┐
│ 投稿内容（左60%）  | 情報    │
│                   |         │
├──────────────────────────────┤
│ 投稿方法選択（左60%）        │
│ ○ すぐに投稿                 │
│ ○ 時間指定の予約投稿         │
│ ○ 次のタイミングで投稿       │
├──────────────────────────────┤
│ [条件付き表示]（左60%）      │
└──────────────────────────────┘
```

**CSS**:
- グリッドレイアウト: 2カラム
- 余白有効活用

#### デスクトップ（>= 1024px）

```
┌────────────────────────────────────────┐
│ 投稿内容（左70%）| 予約一覧（右30%）│
│                 |                   │
├────────────────────────────────────────┤
│ 投稿方法選択（左70%）                  │
│ ○ すぐに投稿                           │
│ ○ 時間指定の予約投稿                   │
│ ○ 次のタイミングで投稿                 │
├────────────────────────────────────────┤
│ [条件付き表示]（左70%）                │
└────────────────────────────────────────┘
```

---

## 3. 実装方針

### 3.1 HTML構造の変更

**削除対象**:
- 既存のタブ要素（タブ形式の「時間指定」「次のタイミング」）
- 「すぐに投稿」ボタン（統合）

**追加対象**:
- ラジオボタングループ（`post-method-group`）
- 条件付き表示エリア#1: `scheduled-controls-area`
- 条件付き表示エリア#2: `next-timing-info-area`

**変更対象**:
- 投稿ボタンを1つに統合（`id="post-button"`）
- イベントハンドラを複数APIに対応

### 3.2 CSSの修正

**新規CSS**:
```css
/* ラジオボタングループのスタイル */
#post-method-group {
  border: none;
  padding: 0;
  margin: 16px 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

#post-method-group legend {
  font-size: 14px;
  font-weight: 600;
  margin-bottom: 12px;
  padding: 0;
}

/* 条件付き表示エリア */
#scheduled-controls-area,
#next-timing-info-area {
  transition: all 0.2s ease-in-out;
  max-height: 500px;
}

#scheduled-controls-area[style*="display: none"],
#next-timing-info-area[style*="display: none"] {
  max-height: 0;
  overflow: hidden;
}

/* レイアウトシフト防止 */
.form-section {
  min-height: auto;  /* 固定高さではなく、内容に応じて変動 */
}
```

**修正対象**:
- 既存の `.schedule-controls` CSS を保持（後方互換性）
- ラジオボタンの視認性向上（背景色、アクティブ状態）
- レイアウトシフト防止（`max-height` トランジション）

### 3.3 JavaScriptロジック

#### 初期化処理

```javascript
// ページロード時
document.addEventListener('DOMContentLoaded', () => {
  // LocalStorageから前回選択を復元
  const savedMethod = localStorage.getItem('post_method_preference') || 'immediate';
  
  // ラジオボタンを選択
  const radio = document.querySelector(`input[name="post_method"][value="${savedMethod}"]`);
  if (radio) {
    radio.checked = true;
    updatePostMethodUI(savedMethod);
  }
});
```

#### ラジオボタン変更イベント

```javascript
// ラジオボタン変更時のイベントハンドラ
document.querySelectorAll('input[name="post_method"]').forEach(radio => {
  radio.addEventListener('change', (e) => {
    const method = e.target.value;
    
    // LocalStorageに保存
    localStorage.setItem('post_method_preference', method);
    
    // UI更新
    updatePostMethodUI(method);
  });
});

function updatePostMethodUI(method) {
  const scheduledArea = document.getElementById('scheduled-controls-area');
  const nextTimingArea = document.getElementById('next-timing-info-area');
  const postButton = document.getElementById('post-button');
  
  // 条件付き表示
  scheduledArea.style.display = method === 'scheduled' ? 'block' : 'none';
  nextTimingArea.style.display = method === 'next_timing' ? 'block' : 'none';
  
  // ボタンラベル更新
  const labels = {
    'immediate': '投稿',
    'scheduled': '予約投稿',
    'next_timing': '次のタイミングで予約'
  };
  postButton.textContent = labels[method] || '投稿';
}
```

#### ボタンクリックイベント

```javascript
// 投稿ボタンクリック時
document.getElementById('post-button').addEventListener('click', async (e) => {
  e.preventDefault();
  
  const method = document.querySelector('input[name="post_method"]:checked').value;
  const formData = new FormData(document.querySelector('form'));
  
  // 投稿方法に応じたエンドポイント
  const endpoints = {
    'immediate': '/api/post',
    'scheduled': '/api/scheduled-posts',
    'next_timing': '/api/scheduled-posts/next'
  };
  
  try {
    const response = await fetch(endpoints[method], {
      method: 'POST',
      body: formData
    });
    
    if (response.ok) {
      const data = await response.json();
      showToast(`投稿を${method === 'immediate' ? '作成' : '予約'}しました。`, 'success');
      // 予約一覧を更新
      refreshScheduledPostsList();
    } else {
      showToast('投稿に失敗しました。', 'error');
    }
  } catch (error) {
    console.error('API Error:', error);
    showToast('エラーが発生しました。', 'error');
  }
});
```

### 3.4 LocalStorage統合

**キー**: `post_method_preference`
**値**: `immediate` | `scheduled` | `next_timing`
**デフォルト**: `immediate`（初回訪問時）

**使用例**:
```javascript
// 保存
localStorage.setItem('post_method_preference', 'next_timing');

// 取得
const method = localStorage.getItem('post_method_preference');

// 削除（オプション）
localStorage.removeItem('post_method_preference');
```

---

## 4. 既存システム統合

### 4.1 API統合仕様

| 投稿方法 | エンドポイント | 呼び出し元 | パラメータ |
|---------|--------------|---------|----------|
| すぐに投稿 | `POST /api/post` | 既存フォーム | content, target_sns, media_files |
| 時間指定 | `POST /api/scheduled-posts` | 既存フォーム + scheduled_at | content, target_sns, media_files, scheduled_at |
| 次のタイミング | `POST /api/scheduled-posts/next` | 新規ロジック | content, target_sns, media_files |

### 4.2 フォーム入力値の保持

**TextArea（投稿内容）**: 既存のフォーム保持機能を使用
**メディアファイル**: 既存のプレビューロジックを使用
**対象SNS**: 既存のチェックボックス機能を使用
**予約日時**: 既存の日時ピッカー機能を使用

### 4.3 エラーハンドリング

**各API失敗時**:
- トーストメッセージ表示
- エラーログ出力
- フォーム状態の保持（再入力可能）

**スロット検索失敗時**（次のタイミング）:
```javascript
// API レスポンス
{
  "created_posts": [...],
  "errors": [
    {"sns": "x", "error": "7日以内に空きスロットが見つかりませんでした"}
  ]
}

// エラー表示
errors.forEach(error => {
  showToast(`${error.sns}: ${error.error}`, 'warning');
});
```

---

## 5. マイグレーション戦略

### 5.1 段階的マイグレーション

**Phase 1: コンポーネント追加**
- ラジオボタングループを新規追加（既存UI保持）
- 新規ロジック実装（既存ロジック保持）

**Phase 2: UIの切り替え**
- 既存タブUIを非表示
- 既存ボタンを非表示
- 新規ラジオボタンUI表示

**Phase 3: クリーンアップ**
- 既存タブUIコード削除
- 既存ボタンロジック削除

### 5.2 後方互換性の維持

- 既存のAPI エンドポイント変更なし
- 既存のフォーム入力フィールド変更なし
- LocalStorageキー固有（既存データに影響なし）

### 5.3 ロールバック計画

変更前のコードをコメント化して保持
```javascript
/* 従来の「すぐに投稿」ボタン - 互換性のため保持
document.getElementById('submit-btn').addEventListener('click', () => {
  // 従来ロジック
});
*/
```

---

## 6. テスト設計

### 6.1 ユニットテスト

**UI状態遷移**:
- ラジオボタン選択時のUI更新
- LocalStorage保存/復元
- ボタンラベル変更

**API呼び出し**:
- 各エンドポイントへの正しい呼び出し
- パラメータの正確性

**エラーハンドリング**:
- API失敗時のエラーメッセージ表示
- スロット検索失敗時の警告表示

### 6.2 統合テスト

**ユースケース検証**:
- UC-1: すぐに投稿 → `/api/post` 呼び出し確認
- UC-2: 時間指定予約 → `/api/scheduled-posts` 呼び出し確認
- UC-3: 次のタイミング → `/api/scheduled-posts/next` 呼び出し確認
- UC-4: 選択状態復元 → LocalStorage復元確認

### 6.3 UI/UXテスト

**レスポンシブ**:
- 各ブレークポイントでのレイアウト確認
- レイアウトシフトなし確認

**アクセシビリティ**:
- キーボードナビゲーション
- スクリーンリーダー対応

---

## 7. 参考: 既存UI との違い

| 項目 | 従来のUI | 改善後のUI |
|------|---------|----------|
| 投稿方法選択 | タブ形式 | ラジオボタン（縦積み） |
| ボタン数 | 2つ（「すぐに投稿」「予約投稿」） | 1つ（「投稿」→ ラベル動的変更） |
| UI安定性 | タブ切り替えでレイアウト変動 | 固定レイアウト（条件付き表示のみ） |
| 状態管理 | ページ内のみ | LocalStorage で永続化 |
| レスポンシブ | 固定 | 3ブレークポイント対応 |
