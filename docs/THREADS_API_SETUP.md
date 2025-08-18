# Threads API 設定手順書

このドキュメントでは、blog-autopostでThreads投稿機能を使用するための詳細な設定手順を説明します。

## 前提条件

- Meta（Facebook）アカウント
- Instagram Business Account または Instagram Creator Account
- Threads アカウント（Instagram アカウントと連携済み）

⚠️ **重要**: 個人のInstagramアカウントではThreads APIを使用できません。Business または Creator アカウントが必要です。

## Step 1: Meta for Developers アプリ作成

### 1.1 Developer Portal にアクセス

1. [Meta for Developers](https://developers.facebook.com/) にアクセス
2. Meta（Facebook）アカウントでログイン
3. 右上の「My Apps」をクリック

### 1.2 新しいアプリの作成

1. 「Create App」をクリック
2. **Use case** で「Other」を選択
3. 「Next」をクリック
4. **App type** で「Business」を選択
5. 「Next」をクリック
6. アプリ情報を入力：
   - **App name**: `blog-autopost-threads`（任意の名前）
   - **App contact email**: あなたのメールアドレス
7. 「Create app」をクリック

### 1.3 Threads API 製品の追加

1. アプリダッシュボードで「Add products to your app」セクションを探す
2. **Threads API** の「Set up」をクリック
3. 基本設定が完了

## Step 2: アプリの基本設定

### 2.1 App domains の設定

1. 左サイドバーで「Settings」→「Basic」をクリック
2. **App Domains** に以下を追加：
   - `localhost`（開発用）
   - あなたのブログドメイン（本番用）

### 2.2 Platform の追加

1. 同じページで「+ Add Platform」をクリック
2. 「Website」を選択
3. **Site URL** に以下を設定：
   - 開発時: `http://localhost:3000`
   - 本番時: あなたのブログURL

## Step 3: 認証設定

### 3.1 OAuth Redirect URIs の設定

1. 左サイドバーで「Threads API」→「Settings」をクリック
2. **Valid OAuth Redirect URIs** に以下を追加：
   ```
   http://localhost:3000/auth/callback
   https://yourdomain.com/auth/callback
   ```

### 3.2 権限の設定

1. 同じページで **Permissions** セクションを確認
2. 以下の権限が有効になっていることを確認：
   - `threads_basic`
   - `threads_content_publish`

## Step 4: アクセストークンの取得

### 4.1 テストユーザーの追加

1. 左サイドバーで「Threads API」→「Tools」をクリック
2. **Add Threads Tester** でInstagramアカウントを追加
3. 追加されたアカウントで承認

### 4.2 短期アクセストークンの生成

1. 「Threads API」→「Tools」→「Access Token Debugger」を使用
2. 「Generate Token」をクリック
3. Instagramアカウントでログイン・認証
4. 短期アクセストークン（1時間有効）を取得

### 4.3 長期アクセストークンへの変換

短期トークンを長期トークン（60日間有効）に変換します：

```bash
curl -i -X GET "https://graph.threads.net/access_token?grant_type=th_exchange_token&client_secret=YOUR_APP_SECRET&access_token=SHORT_LIVED_TOKEN"
```

レスポンス例：
```json
{
  "access_token": "LONG_LIVED_ACCESS_TOKEN",
  "token_type": "bearer",
  "expires_in": 5184000
}
```

## Step 5: config.yml の設定

### 5.1 認証情報の追加

`config.yml` の `sns` セクションに以下を追加：

```yaml
sns:
  - type: threads
    name: "threads"
    app_id: "YOUR_APP_ID"
    app_secret: "YOUR_APP_SECRET"
    access_token: "YOUR_LONG_LIVED_ACCESS_TOKEN"
```

### 5.2 設定値の取得方法

- **App ID**: アプリダッシュボード「Settings」→「Basic」の「App ID」
- **App Secret**: 同じページの「App Secret」（「Show」をクリック）
- **Access Token**: Step 4.3 で取得した長期アクセストークン

## Step 6: 動作確認

### 6.1 テスト投稿

```bash
# Threads限定投稿
just post-text 'テスト投稿です' --sns threads

# 全SNS投稿（Threadsも含む）
just post-text 'テスト投稿です'
```

### 6.2 RSS監視との連携

```bash
# RSS監視からのThreads投稿
uv run -m src.main --sns threads
```

## トラブルシューティング

### エラー: "Invalid access token"

**原因**: アクセストークンが無効または期限切れ

**解決策**:
1. アクセストークンを再生成（Step 4.2-4.3）
2. config.yml を更新

### エラー: "User ID not found"

**原因**: Instagram Business/Creator アカウントが正しく設定されていない

**解決策**:
1. Instagramアカウントの種別を確認
2. Threads テスターとして正しく追加されているか確認

### エラー: "Container creation failed"

**原因**: 投稿内容またはAPIリクエストに問題

**解決策**:
1. テキストが500文字以内か確認
2. 特殊文字や絵文字の使用を確認
3. API制限（24時間で250投稿）に達していないか確認

### 制限事項

- **文字数制限**: 500文字
- **投稿制限**: 24時間で250投稿、1000リプライ
- **メディア**: 現在はテキスト投稿のみサポート
- **リンクカード**: 未サポート

## アクセストークンの更新

長期アクセストークンは60日間有効です。期限が近づいたら以下で更新：

```bash
curl -i -X GET "https://graph.threads.net/refresh_access_token?grant_type=th_refresh_token&access_token=CURRENT_LONG_LIVED_TOKEN"
```

## セキュリティ上の注意

1. **App Secret** は絶対に公開しないでください
2. **Access Token** はローカルのconfig.ymlのみに保存
3. Gitリポジトリにconfig.ymlをコミットしないよう注意
4. 定期的にアクセストークンを更新

## 参考リンク

- [Threads API Documentation](https://developers.facebook.com/docs/threads)
- [Meta for Developers](https://developers.facebook.com/)
- [Threads API Release Notes](https://developers.facebook.com/docs/threads/release-notes)