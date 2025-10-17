# Liquid Design UI仕様書

## プロジェクト概要

Blog AutoPost CLI の Web UI を iOS 26 Liquid Design に基づくモダンなデザインシステムへ移行する。

Zenn記事「[世界でもっともiOS26を再現したCSSライブラリをリリース！](https://zenn.dev/rdlabo/articles/1cac6284b08db2)」に記載された `@rdlabo/ionic-theme-ios26` ライブラリを活用し、ネイティブアプリのような洗練されたUI/UXを実現する。

## 背景・課題

### 現状
- 現在のWeb UIは基本的なBootstrap風デザイン
- モバイル向けのモダンなデザイン要素が不足
- iOS・Android アプリとの統一感がない

### 課題
- ユーザーの期待値が高まっている（iOS 26デザインの浸透）
- ネイティブアプリ的な洗練されたUXが求められる
- ダークモード対応が必須

### 解決策
- `@rdlabo/ionic-theme-ios26` の導入により、iOS 26デザインを正確に再現
- Ionic Framework のWebコンポーネント活用
- Capacitor連携でモバイルアプリ化への将来性確保

---

## iOS 26 Liquid Design の主要変更点

### 1. セーフエリア（Safe Area）
- **従来**（iOS 18）: セーフエリアは空白で埋める
- **変更**（iOS 26）: エッジ効果を使いながら全画面をユーザーに提供
- **実装**: 予約投稿一覧をエッジツーエッジで表示

### 2. フローティングタブ
- **従来**: 横幅いっぱいのタブバー
- **変更**: フローティングスタイルのタブ + FABボタン対応
- **実装**: 下部ナビゲーション表示をフローティング化

### 3. 要素の丸み（Border Radius）
- **従来**: 直角または小さな丸み
- **変更**: より大きく、統一された丸み
- **実装対象**:
  - ボタン（角丸増加）
  - カード（セッション項目）
  - 削除確認ダイアログ
  - チェックボックス

### 4. コンポーネント間の余白
- **変更**: リスト項目間の余白が大きくなる
- **実装**: カード型レイアウトの導入

### 5. ナビゲーションバーのボタン
- **従来**: ラベル付きボタン
- **変更**: アイコンのみに置き換え（個別またはグループ化）
- **実装**: 削除・更新などのアクションアイコン化

### 6. アラート・ダイアログ
- **変更**: より丸みを帯びる、テキスト左揃え
- **実装**: 削除確認ダイアログのスタイル更新

---

## 実装優先度

### Phase 1: 基盤設定（必須）
**優先度: 1** ✅ **完了**

Ionic + Liquid Design テーマの基本セットアップ。

#### タスク一覧
- [x] Task 1.1: npm パッケージのインストール
  - `@ionic/core` インストール
  - `@rdlabo/ionic-theme-ios26` インストール
  - 依存関係の確認
  - **実装**: CDN経由で導入完了

- [x] Task 1.2: CSS統合
  - `src/web/templates/` 配下のベースHTMLを確認
  - Liquid Design CSS をインポート
  - デフォルト変数ファイルの読み込み
  - ダークモード対応ファイル の選択（System推奨）
  - **実装**: すべてCDN経由で統合完了

- [x] Task 1.3: Ionicコンポーネント化
  - 既存HTMLテンプレートを確認
  - Ionic Web Componentsへの段階的移行計画作成
  - **実装**: 主要コンポーネント移行完了

**完了条件:**
- ✅ テーマライブラリが正常に読み込まれる
- ✅ CSSが適用される
- ✅ Ionicコンポーネントが動作する

---

### Phase 2: コンポーネント更新（必須）
**優先度: 1** ✅ **完了**

主要UIコンポーネントのモダン化。

#### タスク一覧
- [x] Task 2.1: ボタンコンポーネント更新
  - 削除ボタンの角丸拡大
  - 「選択したものを削除」ボタンのスタイル更新
  - ホバー・アクティブ状態の改善
  - Ionicボタンコンポーネント活用
  - **実装**: Ionicons統合、::part(native)でスタイル強制適用、ツールチップ追加

- [x] Task 2.2: カード・リスト表示
  - 予約投稿一覧をカード型に変更
  - 項目間の余白拡大
  - 角丸の統一（Liquid Design標準値）
  - リスト項目のタップ効果
  - **実装**: ion-card、ion-listで実装、余白調整完了

- [x] Task 2.3: フォーム要素
  - チェックボックスの角丸化
  - 入力フィールドのスタイル更新
  - フォーカス状態の改善
  - **実装**: ion-checkbox、ion-input、ion-textareaで実装

- [x] Task 2.4: ダイアログ・アラート
  - 削除確認ダイアログの角丸化
  - テキスト左揃え対応
  - ボタンのスタイル統一
  - **実装**: confirm()は保持、完了通知はIonic Toastに置き換え

**完了条件:**
- ✅ 全コンポーネントがLiquid Designガイドラインに準拠
- ✅ ホバー・フォーカス状態が適切に表示
- ✅ モバイルでの見栄えを確認

---

### Phase 3: レイアウト最適化（推奨）
**優先度: 2**

セーフエリア対応とフローティングタブの実装。

#### タスク一覧
- [ ] Task 3.1: セーフエリア対応
  - エッジツーエッジレイアウト実装
  - コンテンツのパディング調整
  - iPhoneノッチ・Dynamic Island対応

- [ ] Task 3.2: フローティングタブ実装
  - 下部ナビゲーションのフローティング化
  - FABボタンスタイルの検討
  - モバイルでのスクロール時動作調整

**完了条件:**
- セーフエリアが正しく処理される
- フローティングタブがモバイルで適切に表示

---

### Phase 4: ダークモード完全対応（推奨）
**優先度: 2**

System設定に基づくダークモード対応。

#### タスク一覧
- [ ] Task 4.1: ダークモードCSS確認
  - `@rdlabo/ionic-theme-ios26` ダークモード設定の読み込み
  - カラースキーム検証

- [ ] Task 4.2: カスタムカラー調整
  - 背景色の確認
  - テキスト色のコントラスト確認
  - エラーメッセージ色の視認性

**完了条件:**
- ライトモード・ダークモードで正しく表示
- テキスト可読性が確保される

---

## 技術スタック

### UI Framework
- **Ionic Framework**: Web Components による モバイルUI再現
- **Liquid Design Theme**: `@rdlabo/ionic-theme-ios26` npm パッケージ

### CSS
- **Base**: Ionic CSS + Liquid Design CSS
- **カスタマイズ**: Shadow Parts API を活用したCSS拡張

### ダークモード
- **方式**: System preference 自動検出
- **ファイル**: `ionic-theme-dark-system.css` を使用

---

## ファイル変更一覧

### 新規作成
- `.kiro/specs/liquid-design-ui.md` - 本仕様書

### 変更対象
- `src/web/templates/index.html`
  - Ionicコンポーネント導入
  - Liquid Design CSS インポート
  - セーフエリア対応マークアップ

- `src/web/static/css/` (新規作成予定)
  - カスタムCSS（Liquid Design拡張）

- `src/web/static/js/` 
  - Ionicコンポーネント連携スクリプト更新

- `package.json` / `pyproject.toml`
  - `@ionic/core` 依存関係追加
  - `@rdlabo/ionic-theme-ios26` 依存関係追加

### 参考
- [Ionic Framework 公式ドキュメント](https://ionicframework.com/docs)
- [Ionic Theme iOS 26 - GitHub](https://github.com/rdlabo-team/ionic-theme-ios26)
- [デモサイト](https://ionic-theme-ios26.netlify.app/)

---

## 完了条件

### Phase 1 完了時
- Ionicライブラリが動作する
- Liquid Design テーマが適用される
- 既存機能が損なわれていない

### Phase 2 完了時
- 全UIコンポーネントがLiquid Design仕様に準拠
- モバイル・デスクトップで適切に表示
- ダークモードで視認性が確保される

### Phase 3 完了時
- セーフエリア対応が完了
- フローティングタブが動作
- Capacitor連携の基盤が整う

### Phase 4 完了時
- ライト・ダークモードの両方が正しく表示
- テキスト可読性が確保される

---

## 参考資料

- Zenn記事: [世界でもっともiOS26を再現したCSSライブラリをリリース！](https://zenn.dev/rdlabo/articles/1cac6284b08db2)
- GitHub: [ionic-theme-ios26](https://github.com/rdlabo-team/ionic-theme-ios26)
- npm: [@rdlabo/ionic-theme-ios26](https://www.npmjs.com/package/@rdlabo/ionic-theme-ios26)
- Ionic 公式: [Theming Guide](https://ionicframework.com/docs/theming/overview)

---

## 注記

- Liquid Effectの完全再現は技術的制限あり（blur等で近似）
- Capacitor連携によるモバイルアプリ化は将来フェーズ
- 既存JSONデータ・SQLite統合との並行作業を想定

---

## 実装履歴

### 2025-10-17: Phase 1 & Phase 2 完了

**実装内容**:
1. Ionic Framework + Liquid Design Theme 統合 (CDN経由)
2. Ionicons 統合
3. 主要コンポーネントのIonic Web Components化
4. ボタンスタイリング (::part(native)でShadow DOM対応)
5. Toast通知システム実装 (alert()からIonic Toastへ)
6. ツールチップ追加
7. 日時選択UIの改善
8. スペーシング調整

**完了したタスク**:
- Phase 1: Task 1.1, 1.2, 1.3
- Phase 2: Task 2.1, 2.2, 2.3, 2.4

**残存課題**:
- 文字数カウンター表示問題 (保留)
- Phase 3: セーフエリア対応、フローティングタブ (未着手)
- Phase 4: ダークモード完全検証 (未着手)

**コミット**: 1d9beef - `[feat] Ionic Web Components によるUI改善とToast通知の実装`

**メモリ**: `liquid-design-ui-toast-implementation`

