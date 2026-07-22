# Requirements Document: unified-post-form-ui

## Project Description (Input)

UIの統合をお願いします。『すぐに投稿』ボタンの下に「時間指定の予約投稿」および「次のタイミングで投稿」がありますが、レスポンシブルデザインで幅広い状態でのレイアウトだと、選択によりUI位置が大きく変動するという問題があるのと、投稿ボタンが2つ(すぐに投稿、予約)があるのも問題だと思います。そこで「すぐに投稿」「時間指定の予約投稿」「次のタイミングで投稿」を選ぶ形にして、『予約投稿』ボタンが選んでいる内容により変化する形にしたほうが良いかもしれません。初期状態は以前選んでいたものを再度選べるようにするか、固定にするなら『すぐに投稿』でいいかと思います。

## Current Issues

1. **UI不安定性**: タブ切り替えにより、下部要素の位置が大きく変動
2. **ボタン重複**: 「すぐに投稿」と「予約投稿」の2つボタンが存在
3. **選択と実行の分離**: 選択方法とボタンアクションが直感的でない
4. **レスポンシブ問題**: 幅広い画面では余白が生じ、UI効率が低い

## 機能要件 (Functional Requirements)

### FR-1: 投稿方法の統一選択
- ユーザーは「すぐに投稿」「時間指定の予約投稿」「次のタイミングで投稿」の3つの方法を **1つのラジオボタングループ** で選択可能
- 選択状態は画面内に常に視認可能
- 初期状態はLocalStorageから前回選択を復元、または「すぐに投稿」をデフォルト

### FR-2: 投稿ボタンの統一
- 複数存在する投稿ボタン（「すぐに投稿」「予約投稿」）を **1つの「投稿」ボタン** に統合
- ボタンのラベルは選択方法に応じて変動
  - 「すぐに投稿」選択時: ボタンラベル = 「投稿」
  - 「時間指定の予約投稿」選択時: ボタンラベル = 「予約投稿」
  - 「次のタイミングで投稿」選択時: ボタンラベル = 「次のタイミングで予約」

### FR-3: 条件付きUI表示
- 投稿方法の選択に応じて、以下の要素の表示/非表示を制御
  - **「時間指定の予約投稿」選択時**: 日時ピッカー + トレランス説明表示
  - **「次のタイミングで投稿」選択時**: スロット検索説明表示
  - **「すぐに投稿」選択時**: 上記要素すべて非表示

### FR-4: 状態管理
- 選択方法をLocalStorageに保存（キー: `post_method_preference`）
- ページ遷移後も選択状態を保持

### FR-5: API呼び出しの動的制御
- ボタンクリック時に選択方法に応じたエンドポイントを呼び出し
  - 「すぐに投稿」: `POST /api/post`
  - 「時間指定の予約投稿」: `POST /api/scheduled-posts`
  - 「次のタイミングで投稿」: `POST /api/scheduled-posts/next`

---

## ユースケース (Use Cases)

### UC-1: 即座に投稿したい場合
**シナリオ**: ユーザーが速報記事をすぐに投稿したい

1. ラジオボタングループで「すぐに投稿」を選択
2. テキスト・メディア・対象SNSを入力
3. 「投稿」ボタンをクリック
4. すぐに各SNSへ投稿される
5. 完了メッセージ表示

**期待される状態**: 日時ピッカーは非表示、「投稿」ボタンが即座投稿を実行

### UC-2: 特定の時刻に投稿したい場合
**シナリオ**: ユーザーが明日の朝9時に投稿予約したい

1. ラジオボタングループで「時間指定の予約投稿」を選択
2. テキスト・メディア・対象SNS・予約日時を入力
3. 「予約投稿」ボタンをクリック
4. 指定日時に予約投稿が作成される
5. 予約一覧に追加される

**期待される状態**: 日時ピッカーが表示、ボタンラベル「予約投稿」

### UC-3: 最適な投稿タイミングを自動選択したい場合
**シナリオ**: ユーザーが各SNSの最適タイミングに自動予約したい

1. ラジオボタングループで「次のタイミングで投稿」を選択
2. テキスト・メディア・対象SNSを入力
3. 「次のタイミングで予約」ボタンをクリック
4. システムが各SNS毎に次の空きスロットを検索
5. スロット見つかった分だけ予約投稿が作成される

**期待される状態**: 日時ピッカーは非表示、スロット検索説明表示

### UC-4: 前回の選択を記憶したい場合
**シナリオ**: ユーザーが普段「次のタイミングで投稿」を使用

1. 前回「次のタイミングで投稿」を選択したまま離脱
2. ページを再度開く
3. 「次のタイミングで投稿」が自動選択されている
4. 入力フォームがそれに応じた状態で表示されている

**期待される状態**: LocalStorageから復元、UI状態が一貫

---

## 受け入れ条件 (Acceptance Criteria)

### AC-1: ラジオボタングループ
- [ ] 3つのラジオボタン（「すぐに投稿」「時間指定」「次のタイミング」）が表示される
- [ ] 同時に1つのみ選択可能
- [ ] 選択状態が視認可能（チェックマーク、背景色など）

### AC-2: UI表示制御
- [ ] 「時間指定」選択時、日時ピッカーが表示される
- [ ] 「次のタイミング」選択時、スロット検索説明が表示される
- [ ] 「すぐに投稿」選択時、上記要素がすべて非表示
- [ ] 選択変更時、UIが即座に更新（遅延なし）

### AC-3: ボタン統一
- [ ] 投稿ボタンが1つのみ存在
- [ ] ボタンラベルが選択方法に応じて変更される
  - 「すぐに投稿」: 「投稿」
  - 「時間指定」: 「予約投稿」
  - 「次のタイミング」: 「次のタイミングで予約」

### AC-4: API呼び出し
- [ ] 「すぐに投稿」選択時、`POST /api/post` が呼ばれる
- [ ] 「時間指定」選択時、`POST /api/scheduled-posts` が呼ばれる
- [ ] 「次のタイミング」選択時、`POST /api/scheduled-posts/next` が呼ばれる
- [ ] エラーハンドリング: 各API失敗時のエラーメッセージ表示

### AC-5: 状態永続化
- [ ] 選択方法がLocalStorageに保存される
- [ ] ページリロード後も選択状態が復元される
- [ ] 初期訪問時はデフォルト「すぐに投稿」が選択

### AC-6: レスポンシブ対応
- [ ] モバイル（< 768px）: ラジオボタンが適切に配置
- [ ] タブレット（768px - 1024px）: 余白が有効活用される
- [ ] デスクトップ（>= 1024px）: 全要素がバランスよく配置
- [ ] 選択変更時、レイアウトシフトなし

### AC-7: アクセシビリティ
- [ ] ラジオボタンが `<input type="radio">` で実装
- [ ] ラベルが `<label>` と関連付けられている
- [ ] キーボードナビゲーション対応（Tab/矢印キー）
- [ ] スクリーンリーダー対応

---

## 技術仕様 (Technical Specifications)

### TS-1: UI状態遷移
```
初期状態: LocalStorageから復元 OR 「すぐに投稿」
    ↓
ユーザーがラジオボタン選択
    ↓
変更イベント発火 → LocalStorageに保存 → UI更新
    ↓
ボタンクリック → 選択方法に応じたAPI呼び出し
```

### TS-2: DOM構造
```html
<!-- ラジオボタングループ -->
<fieldset id="post-method-group">
  <legend>投稿方法を選択</legend>
  <label><input type="radio" name="post_method" value="immediate" /> すぐに投稿</label>
  <label><input type="radio" name="post_method" value="scheduled" /> 時間指定の予約投稿</label>
  <label><input type="radio" name="post_method" value="next_timing" /> 次のタイミングで投稿</label>
</fieldset>

<!-- 条件付き表示エリア -->
<div id="scheduled-controls" style="display: none;">
  <!-- 日時ピッカー -->
</div>

<div id="next-timing-info" style="display: none;">
  <!-- スロット検索説明 -->
</div>

<!-- ボタン -->
<button id="post-button">投稿</button>
```

### TS-3: JavaScriptロジック
```javascript
// ラジオボタン変更時
document.querySelectorAll('input[name="post_method"]').forEach(radio => {
  radio.addEventListener('change', () => {
    const method = radio.value;
    localStorage.setItem('post_method_preference', method);
    updateUI(method);
  });
});

// UI更新
function updateUI(method) {
  const scheduledControls = document.getElementById('scheduled-controls');
  const nextTimingInfo = document.getElementById('next-timing-info');
  const postButton = document.getElementById('post-button');
  
  scheduledControls.style.display = method === 'scheduled' ? 'block' : 'none';
  nextTimingInfo.style.display = method === 'next_timing' ? 'block' : 'none';
  
  postButton.textContent = {
    'immediate': '投稿',
    'scheduled': '予約投稿',
    'next_timing': '次のタイミングで予約'
  }[method];
}

// ボタンクリック時
document.getElementById('post-button').addEventListener('click', () => {
  const method = document.querySelector('input[name="post_method"]:checked').value;
  
  const endpoints = {
    'immediate': '/api/post',
    'scheduled': '/api/scheduled-posts',
    'next_timing': '/api/scheduled-posts/next'
  };
  
  submitPost(endpoints[method]);
});
```

### TS-4: LocalStorage
- キー: `post_method_preference`
- 値: `immediate` | `scheduled` | `next_timing`
- 初期値: なし（初回訪問時は「すぐに投稿」）

### TS-5: API呼び出し仕様
| 投稿方法 | エンドポイント | パラメータ |
|---------|--------------|----------|
| すぐに投稿 | `POST /api/post` | content, target_sns, media_files |
| 時間指定 | `POST /api/scheduled-posts` | content, target_sns, media_files, scheduled_at |
| 次のタイミング | `POST /api/scheduled-posts/next` | content, target_sns, media_files |

---

## 非機能要件 (Non-Functional Requirements)

### NFR-1: パフォーマンス
- UI更新（選択変更）: 100ms以内
- API呼び出し: 3秒以内にタイムアウト

### NFR-2: レスポンシブ対応
- モバイル: 320px以上対応
- タブレット: 768px以上対応
- デスクトップ: 1024px以上対応

### NFR-3: ブラウザ互換性
- Chrome, Firefox, Safari, Edge の最新2バージョン
- モバイルブラウザ対応

### NFR-4: アクセシビリティ
- WCAG 2.1 AA準拠
- キーボード操作のみで全機能利用可能

---

## Reference Screenshots

- Current UI: タブ形式で「時間指定の予約投稿」「次のタイミングで投稿」を切り替え
- 問題: タブ選択時にUI レイアウトが大きく変動

---

## 実装との差異メモ (2026-07-22)

本specは設計時の想定として `name="post_method"`、値を `immediate` / `scheduled` /
`next_timing` と記述しているが、**実際の実装は異なる命名になっている**。
機能としては要件を満たしているため、完了判定は変更しない。

| 項目 | spec の記述 | `static/index.html` の実装 |
|---|---|---|
| ラジオの name | `post_method` | `schedule-type` |
| 値(すぐに投稿) | `immediate` | `now` |
| 値(時間指定) | `scheduled` | `custom` |
| 値(次のタイミング) | `next_timing` | `next` |
| 条件付き表示の制御 | `setMethod()` / `postState` | `toggleCustomDate(bool)` |
| 日時入力欄のID | `scheduled-controls-area` | `custom-date-container` |

APIへ送るキーは `schedule_type` で、値は `now` / `custom` / `next`。
`src/web/routes.rs` の `ManualPostRequest.schedule_type` が受け取る。

**注意**: `post_method` という名前は、配信されない死にコードだった
`src/web/templates/index.html` 側で使われていたもの。実装を確認する際は
必ず `static/index.html` を見ること。

