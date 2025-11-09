# Tasks Document: unified-post-form-ui

実装タスク一覧。各タスクはTDD（テスト駆動開発）に従い、テストコードを先に作成してから実装を行う。

---

## Phase 1: 準備フェーズ

### Task 1.1: 既存UI調査・ドキュメント化

**目的**: 現在のUI構造を理解し、削除対象・変更対象を明確にする

**実装内容**:
1. `src/web/templates/index.html` の以下を確認
   - 「すぐに投稿」ボタンの位置・ID
   - 「予約投稿」ボタンの位置・ID
   - 既存タブUI（時間指定・次のタイミング）の構造
   - 既存日時ピッカーの実装
2. 既存JavaScriptロジックを確認
   - ボタンクリックハンドラ
   - フォーム送信ロジック
   - API呼び出し箇所
3. 既存スタイルを確認
   - `.schedule-controls` CSS
   - `.schedule-item` CSS
   - レイアウト関連CSS

**検証方法**:
- [ ] 既存コード構造をドキュメント化した
- [ ] 削除対象コンポーネントを特定した
- [ ] 変更対象コンポーネントを特定した

**推定時間**: 30分

**依存タスク**: なし

---

### Task 1.2: 開発ブランチ作成

**目的**: メイン分岐から独立したブランチで開発を行う

**実装内容**:
```bash
git checkout main
git pull origin main
git checkout -b feature/unified-post-form-ui
```

**検証方法**:
- [ ] ブランチが `feature/unified-post-form-ui` に切り替わった
- [ ] `git status` でクリーンな状態確認
- [ ] `git log` で最新のmainコミットが確認できる

**推定時間**: 5分

**依存タスク**: なし

---

## Phase 2: HTML/CSS実装

### Task 2.1: ラジオボタングループHTML追加

**目的**: 投稿方法を選択するラジオボタングループを追加

**実装内容**:
1. `src/web/templates/index.html` 内、「予約投稿」セクションの前に以下を挿入:
   ```html
   <div class="post-method-selector">
     <fieldset id="post-method-group">
       <legend>投稿方法を選択</legend>
       
       <ion-item lines="none">
         <ion-label>
           <input type="radio" name="post_method" value="immediate" checked />
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
   </div>
   ```

2. 既存の日時ピッカーセクションを `id="scheduled-controls-area"` でラップ

3. 既存のスロット検索説明を新規 `id="next-timing-info-area"` でラップ

**検証方法**:
- [ ] HTMLが正しく挿入された
- [ ] ラジオボタンが3つ表示されている
- [ ] ラジオボタンが同時に1つのみ選択可能
- [ ] 各ラベルテキストが正確

**推定時間**: 20分

**依存タスク**: Task 1.1

---

### Task 2.2: 条件付き表示エリアHTML整備

**目的**: 日時ピッカーとスロット検索説明が条件に応じて表示/非表示になるようにする

**実装内容**:
1. 既存の日時ピッカーセクションを以下でラップ:
   ```html
   <div id="scheduled-controls-area" style="display: none;">
     <!-- 既存の日時ピッカー -->
   </div>
   ```

2. 既存のスロット検索説明（タブまたはセクション）を以下でラップ:
   ```html
   <div id="next-timing-info-area" style="display: none;">
     <!-- 既存のスロット検索説明 -->
   </div>
   ```

3. 既存の「すぐに投稿」ボタンと「予約投稿」ボタンを以下に統合:
   ```html
   <ion-button id="post-button" expand="block" type="submit">
     <ion-icon slot="start" name="send"></ion-icon>
     <span id="post-button-label">投稿</span>
   </ion-button>
   ```

**検証方法**:
- [ ] 日時ピッカーが `id="scheduled-controls-area"` でラップされている
- [ ] スロット検索説明が `id="next-timing-info-area"` でラップされている
- [ ] ボタンが1つに統合されている
- [ ] 初期状態で日時ピッカーと説明が非表示

**推定時間**: 20分

**依存タスク**: Task 2.1

---

### Task 2.3: CSS修正（レイアウト・視認性）

**目的**: ラジオボタングループのスタイリング、条件付き表示エリアのトランジション

**実装内容**:
1. `<style>` セクションに以下のCSSを追加:
   ```css
   /* ラジオボタングループ */
   .post-method-selector {
     margin: 16px 0;
   }
   
   #post-method-group {
     border: none;
     padding: 0;
     margin: 0;
     display: flex;
     flex-direction: column;
     gap: 8px;
   }
   
   #post-method-group legend {
     font-size: 14px;
     font-weight: 600;
     margin-bottom: 12px;
     padding: 0;
     color: var(--ion-color-medium-shade);
   }
   
   #post-method-group ion-item {
     --padding-start: 12px;
     --padding-end: 12px;
   }
   
   #post-method-group input[type="radio"]:checked + span {
     font-weight: 600;
     color: var(--ion-color-primary);
   }
   
   /* 条件付き表示エリア */
   #scheduled-controls-area,
   #next-timing-info-area {
     transition: all 0.2s ease-in-out;
     overflow: hidden;
   }
   
   #scheduled-controls-area[style*="block"],
   #next-timing-info-area[style*="block"] {
     animation: slideDown 0.2s ease-out;
   }
   
   @keyframes slideDown {
     from {
       opacity: 0;
       max-height: 0;
     }
     to {
       opacity: 1;
       max-height: 500px;
     }
   }
   ```

2. 既存の `.schedule-controls` CSS を保持（後方互換性）

**検証方法**:
- [ ] ラジオボタンが視認可能なスタイルで表示
- [ ] 選択状態が視覚的に区別される
- [ ] 条件付き表示がスムーズなトランジション
- [ ] レイアウトシフトが発生しない

**推定時間**: 30分

**依存タスク**: Task 2.2

---

## Phase 3: JavaScript実装

### Task 3.1: 状態管理ロジック実装

**目的**: グローバル状態と LocalStorage 管理を実装

**実装内容**:
1. HTMLの `<script>` 内に以下を追加:
   ```javascript
   // グローバル状態
   const postState = {
     method: 'immediate',  // immediate | scheduled | next_timing
     init: function() {
       // LocalStorageから前回選択を復元
       const saved = localStorage.getItem('post_method_preference');
       this.method = saved || 'immediate';
     },
     setMethod: function(method) {
       this.method = method;
       localStorage.setItem('post_method_preference', method);
     },
     getMethod: function() {
       return this.method;
     }
   };
   
   // ページロード時の初期化
   document.addEventListener('DOMContentLoaded', () => {
     postState.init();
   });
   ```

**検証方法**:
- [ ] `postState` オブジェクトが作成されている
- [ ] `init()` メソッドがLocalStorageから値を読み込める
- [ ] `setMethod()` がLocalStorageに保存できる
- [ ] ページリロード後も値が保持される

**推定時間**: 15分

**依存タスク**: Task 2.3

---

### Task 3.2: ラジオボタン変更イベント実装

**目的**: ラジオボタン選択時に状態を更新し、UIを変更

**実装内容**:
1. HTMLの `<script>` 内に以下を追加:
   ```javascript
   // ラジオボタン変更イベント
   document.querySelectorAll('input[name="post_method"]').forEach(radio => {
     radio.addEventListener('change', (e) => {
       const method = e.target.value;
       postState.setMethod(method);
       updatePostMethodUI(method);
       
       console.log(`投稿方法を切り替え: ${method}`);
     });
   });
   ```

2. ページロード時に保存された方法でラジオボタンを選択:
   ```javascript
   document.addEventListener('DOMContentLoaded', () => {
     postState.init();
     
     // 保存されたラジオボタンを選択
     const radio = document.querySelector(
       `input[name="post_method"][value="${postState.getMethod()}"]`
     );
     if (radio) {
       radio.checked = true;
       updatePostMethodUI(postState.getMethod());
     }
   });
   ```

**検証方法**:
- [ ] ラジオボタン選択時にイベントが発火
- [ ] `postState.method` が更新される
- [ ] LocalStorageに保存される
- [ ] ページリロード後にラジオボタンが復元される
- [ ] コンソールに適切なログが出力される

**推定時間**: 20分

**依存タスク**: Task 3.1

---

### Task 3.3: UI更新ロジック実装

**目的**: ラジオボタン選択に応じて条件付きUI要素を表示/非表示にする

**実装内容**:
1. HTMLの `<script>` 内に以下を追加:
   ```javascript
   function updatePostMethodUI(method) {
     const scheduledArea = document.getElementById('scheduled-controls-area');
     const nextTimingArea = document.getElementById('next-timing-info-area');
     const postButton = document.getElementById('post-button');
     const buttonLabel = document.getElementById('post-button-label');
     
     // 条件付き表示
     scheduledArea.style.display = method === 'scheduled' ? 'block' : 'none';
     nextTimingArea.style.display = method === 'next_timing' ? 'block' : 'none';
     
     // ボタンラベル更新
     const labels = {
       'immediate': '投稿',
       'scheduled': '予約投稿',
       'next_timing': '次のタイミングで予約'
     };
     
     if (buttonLabel) {
       buttonLabel.textContent = labels[method] || '投稿';
     } else {
       postButton.innerHTML = `<ion-icon slot="start" name="send"></ion-icon>${labels[method] || '投稿'}`;
     }
     
     console.log(`UI更新: ${method}`);
   }
   ```

2. レイアウトシフト防止のため、条件付きエリアの高さ設定を確認

**検証方法**:
- [ ] 「すぐに投稿」選択時、両方の条件付きエリアが非表示
- [ ] 「時間指定」選択時、日時ピッカーが表示、説明は非表示
- [ ] 「次のタイミング」選択時、説明が表示、日時ピッカーは非表示
- [ ] ボタンラベルが正しく変更される
- [ ] トランジションがスムーズ
- [ ] レイアウトシフトなし

**推定時間**: 25分

**依存タスク**: Task 3.2

---

### Task 3.4: ボタンクリックイベント実装

**目的**: 統合ボタンクリック時に、選択方法に応じたAPIを呼び出す

**実装内容**:
1. HTMLの `<script>` 内に以下を追加:
   ```javascript
   document.getElementById('post-button').addEventListener('click', async (e) => {
     e.preventDefault();
     
     const method = postState.getMethod();
     const form = document.querySelector('form');  // フォーム要素を特定
     const formData = new FormData(form);
     
     // 投稿方法に応じたエンドポイント
     const endpoints = {
       'immediate': '/api/post',
       'scheduled': '/api/scheduled-posts',
       'next_timing': '/api/scheduled-posts/next'
     };
     
     const endpoint = endpoints[method];
     
     try {
       console.log(`API呼び出し: ${endpoint} (${method})`);
       
       const response = await fetch(endpoint, {
         method: 'POST',
         body: formData
       });
       
       if (response.ok) {
         const data = await response.json();
         
         // 成功メッセージ
         const message = method === 'immediate' 
           ? '投稿を作成しました。'
           : method === 'scheduled'
           ? '投稿を予約しました。'
           : '次のタイミングで予約投稿を作成しました。';
         
         showToast(message, 'success');
         
         // 予約一覧更新
         if (method !== 'immediate') {
           refreshScheduledPostsList();
         }
         
         // フォームリセット
         form.reset();
       } else {
         const errorData = await response.json();
         showToast(`エラー: ${errorData.detail || '投稿に失敗しました。'}`, 'error');
       }
     } catch (error) {
       console.error('API Error:', error);
       showToast('ネットワークエラーが発生しました。', 'error');
     }
   });
   ```

2. 既存の個別ボタンイベントハンドラを削除

**検証方法**:
- [ ] 「すぐに投稿」選択 → `/api/post` が呼ばれている
- [ ] 「時間指定」選択 → `/api/scheduled-posts` が呼ばれている
- [ ] 「次のタイミング」選択 → `/api/scheduled-posts/next` が呼ばれている
- [ ] 成功時にトーストメッセージが表示される
- [ ] エラー時に適切なエラーメッセージが表示される
- [ ] ネットワークエラー時に通知される
- [ ] コンソールにAPIログが出力される

**推定時間**: 30分

**依存タスク**: Task 3.3

---

## Phase 4: 統合・テスト

### Task 4.1: 既存コンポーネント削除

**目的**: 従来のUI（タブ、個別ボタン）を削除し、スッキリさせる

**実装内容**:
1. HTML内の以下の要素を削除:
   - 従来の「すぐに投稿」ボタン
   - 従来のタブUI（時間指定・次のタイミングの表示切り替え）
   - 従来の「予約投稿」ボタン

2. 削除前にコメント化して保持（ロールバック用）

3. 既存JavaScriptイベントハンドラを削除

**検証方法**:
- [ ] HTMLが削除された
- [ ] 従来のボタンが表示されない
- [ ] 従来のタブが表示されない
- [ ] ページが正常に読み込まれる
- [ ] コンソールにエラーが出ない

**推定時間**: 20分

**依存タスク**: Task 3.4

---

### Task 4.2: API呼び出し統合テスト

**目的**: 各API呼び出しが正しく動作することを確認

**実装内容**:
1. ブラウザ開発者ツールを使用してテスト:
   - Network タブで API呼び出しを確認
   - Console タブでエラーを確認

2. 各投稿方法でテスト:
   - **すぐに投稿**: `/api/post` が呼ばれ、即座に投稿される
   - **時間指定**: `/api/scheduled-posts` が呼ばれ、予約投稿が作成される
   - **次のタイミング**: `/api/scheduled-posts/next` が呼ばれ、複数の予約が作成される

3. パラメータの正確性を確認:
   - `content` が正しく送信される
   - `target_sns` が正しく送信される
   - `media_files` が正しく送信される（ある場合）
   - `scheduled_at` が正しい形式で送信される（時間指定の場合）

**検証方法**:
- [ ] ネットワークリクエストが正しいエンドポイントに送信されている
- [ ] ステータスコード 200/201 が返されている
- [ ] レスポンスデータが正しい形式
- [ ] 予約一覧が更新されている
- [ ] トーストメッセージが表示される

**推定時間**: 30分

**依存タスク**: Task 4.1

---

### Task 4.3: エラーハンドリングテスト

**目的**: エラーケースでの動作を確認

**実装内容**:
1. ブラウザ開発者ツールを使用してテスト:
   - Network > Throttling で「Offline」を選択
   - 投稿ボタンをクリック
   - エラーメッセージが表示されることを確認

2. 各APIのエラーレスポンスをシミュレート:
   - 400 Bad Request
   - 401 Unauthorized
   - 500 Internal Server Error

3. スロット検索失敗時の動作:
   - `next_timing` で7日以内にスロットがない場合
   - エラーメッセージが表示されることを確認

**検証方法**:
- [ ] ネットワークエラー時にメッセージが表示される
- [ ] APIエラー時にメッセージが表示される
- [ ] スロット検索失敗時にメッセージが表示される
- [ ] フォームがリセットされない（再入力可能）
- [ ] ボタンが無効化されない（再試行可能）

**推定時間**: 25分

**依存タスク**: Task 4.2

---

### Task 4.4: UIテスト（レスポンシブ・アクセシビリティ）

**目的**: 各ブレークポイントとアクセシビリティを確認

**実装内容**:
1. **レスポンシブテスト**（ブラウザ開発者ツール）:
   - モバイル (375px): ラジオボタンが積み重なって表示
   - タブレット (768px): ラジオボタンが横に並ぶ（レイアウト確認）
   - デスクトップ (1024px以上): ラジオボタンが整列

2. **アクセシビリティテスト**:
   - キーボード操作: Tab キーでラジオボタンに移動可能
   - 矢印キー: ↑↓ キーでラジオボタンを切り替え可能
   - スクリーンリーダー（NVDA/JAWS）: ラベルが読み上げられることを確認

3. **視認性テスト**:
   - 選択状態が視覚的に区別される
   - ボタンラベルが正しく変更される
   - トランジションが滑らか
   - レイアウトシフトなし

**検証方法**:
- [ ] 各ブレークポイントでレイアウトが正常
- [ ] キーボードナビゲーション可能
- [ ] スクリーンリーダー対応
- [ ] 視認性に問題なし
- [ ] トランジション時にレイアウトシフトなし

**推定時間**: 40分

**依存タスク**: Task 4.3

---

## Phase 5: マージ・リリース

### Task 5.1: コード品質チェック・マージ

**目的**: コードの品質を確認し、mainブランチにマージ

**実装内容**:
1. **Linting チェック**:
   ```bash
   npm run lint  # または既存のlint コマンド
   ```
   エラーを修正する

2. **ブラウザテスト** (最終確認):
   - Chrome、Firefox、Safari で動作確認
   - ネットワーク遅延シミュレーション

3. **マージ準備**:
   ```bash
   git add .
   git commit -m "[feat] unified-post-form-ui: 投稿方法選択UI統合"
   git push origin feature/unified-post-form-ui
   ```

4. **Pull Request 作成**:
   - GitHub で PR を作成
   - テスト結果をまとめて記載
   - レビュー依頼

5. **マージ**:
   ```bash
   git checkout main
   git merge --ff-only feature/unified-post-form-ui
   git push origin main
   ```

6. **ブランチ削除**:
   ```bash
   git branch -d feature/unified-post-form-ui
   ```

**検証方法**:
- [ ] Lint エラーがない
- [ ] 各ブラウザで動作確認
- [ ] PR がレビュー承認される
- [ ] main ブランチへマージ成功
- [ ] デプロイ後も動作確認

**推定時間**: 30分

**依存タスク**: Task 4.4

---

## テスト検証チェックリスト

### ユースケース検証

- [ ] **UC-1**: すぐに投稿
  - ラジオボタンで「すぐに投稿」を選択
  - 内容を入力
  - 「投稿」ボタンをクリック
  - `/api/post` が呼ばれることを確認
  - トーストメッセージが表示される

- [ ] **UC-2**: 時間指定予約
  - ラジオボタンで「時間指定」を選択
  - 日時ピッカーが表示される
  - 日時を選択
  - 「予約投稿」ボタンをクリック
  - `/api/scheduled-posts` が呼ばれることを確認
  - 予約一覧に追加される

- [ ] **UC-3**: 次のタイミング投稿
  - ラジオボタンで「次のタイミング」を選択
  - スロット検索説明が表示される
  - 「次のタイミングで予約」ボタンをクリック
  - `/api/scheduled-posts/next` が呼ばれることを確認
  - 複数の予約が作成される

- [ ] **UC-4**: 選択状態復元
  - 「次のタイミング」を選択
  - ページをリロード
  - ラジオボタンが「次のタイミング」のまま
  - UIが前回の状態で表示される

### 非機能要件検証

- [ ] **パフォーマンス**: UI更新 100ms以内
- [ ] **レスポンシブ**: 3ブレークポイント全対応
- [ ] **ブラウザ互換**: Chrome/Firefox/Safari/Edge の最新版で動作
- [ ] **アクセシビリティ**: WCAG 2.1 AA準拠

---

## 参考: 既存APIエンドポイント

| API | メソッド | パスパラメータ | 説明 |
|-----|---------|---------------|------|
| `/api/post` | POST | - | 即時投稿 |
| `/api/scheduled-posts` | POST | - | 予約投稿作成 |
| `/api/scheduled-posts/next` | POST | - | 次のタイミング投稿 |
| `/api/scheduled-posts` | GET | - | 予約投稿一覧 |

---

## 実装進捗トラッキング

初期状態: すべてのタスクが `pending` (未開始)

進捗マップ:
```
Phase 1: [準備]         ████░░░░░░  20%
Phase 2: [HTML/CSS]     ░░░░░░░░░░   0%
Phase 3: [JavaScript]   ░░░░░░░░░░   0%
Phase 4: [統合・テスト] ░░░░░░░░░░   0%
Phase 5: [マージ]       ░░░░░░░░░░   0%
-----------------------------------------
全体進捗:               ░░░░░░░░░░   0%
```

各タスク完了時に更新。
