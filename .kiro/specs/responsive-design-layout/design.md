# Technical Design Document

## Overview

レスポンシブデザイン対応による UI レイアウトの再構成。デスクトップ環境では左側に新規投稿フォーム、右側に予約投稿一覧を配置し、操作性を向上させる。

## Architecture

### 1. グリッドレイアウト構造

#### デスクトップ（≥1024px）
```
┌─────────────────────────────────────────┐
│ Header                                  │
├──────────────────┬──────────────────────┤
│                  │                      │
│  Left Column     │  Right Column        │
│  (投稿フォーム)   │  (予約投稿一覧)      │
│                  │  [固定]              │
│  max-width: 45%  │  width: 55%          │
│                  │                      │
│  - 新規投稿       │  - ソート            │
│  - 予約投稿       │  - 一括削除          │
│                  │  - テーブル          │
│                  │  - スクロール可      │
│                  │                      │
└──────────────────┴──────────────────────┘
```

#### タブレット（768px ≤ 幅 < 1024px）
```
┌─────────────────────────────────┐
│ Header                          │
├─────────────────────────────────┤
│  新規投稿フォーム               │
│  - 全幅表示                     │
├─────────────────────────────────┤
│  予約投稿一覧                   │
│  - スクロール可能               │
└─────────────────────────────────┘
```

#### モバイル（<768px）
```
┌──────────────┐
│ Header       │
├──────────────┤
│ 新規投稿     │
│ フォーム     │
│ (全幅)       │
├──────────────┤
│ 予約投稿     │
│ 一覧         │
│ (横スクロール)
└──────────────┘
```

### 2. CSS ブレークポイント戦略

#### 定義するブレークポイント
- **Mobile**: < 768px
- **Tablet**: 768px ≤ 幅 < 1024px
- **Desktop**: ≥ 1024px

#### 実装方針
- モバイルファースト設計
- CSS Grid + Flexbox の組み合わせ
- メディアクエリで段階的に対応

### 3. 具体的な実装

#### ion-app コンテナ
```css
/* モバイル・タブレット: 単一カラム */
ion-content {
  display: flex;
  flex-direction: column;
}

/* デスクトップ: 2カラムレイアウト */
@media (min-width: 1024px) {
  ion-content {
    display: grid;
    grid-template-columns: 1fr 1.2fr;
    gap: 20px;
  }
}
```

#### 左カラム（新規投稿フォーム）
```css
#post-form {
  /* モバイル・タブレット */
  width: 100%;
  
  /* デスクトップ */
  @media (min-width: 1024px) {
    max-width: 100%;
    order: 1;
    overflow-y: auto;
    max-height: calc(100vh - 120px);
  }
}
```

#### 右カラム（予約投稿一覧）
```css
.scheduled-posts-section {
  /* モバイル・タブレット */
  width: 100%;
  order: 2;
  
  /* デスクトップ */
  @media (min-width: 1024px) {
    position: relative;
    order: 2;
    width: 100%;
    max-height: calc(100vh - 120px);
    overflow-y: auto;
    border-left: 1px solid var(--ion-color-light);
    padding-left: 20px;
  }
}
```

#### テーブルレスポンシブ対応
```css
/* モバイル: 横スクロール */
@media (max-width: 767px) {
  .table-wrapper {
    overflow-x: auto;
    -webkit-overflow-scrolling: touch;
  }
  
  table {
    min-width: 800px;
  }
}

/* デスクトップ: 通常表示 */
@media (min-width: 1024px) {
  .table-wrapper {
    overflow-x: visible;
  }
  
  table {
    width: 100%;
  }
}
```

#### 入力フィールド対応
```css
/* モバイル・タブレット: 全幅 */
ion-item, ion-input, ion-textarea {
  width: 100%;
}

/* デスクトップ: 複数列対応（将来） */
@media (min-width: 1024px) {
  .form-row {
    display: flex;
    gap: 12px;
  }
  
  .form-row ion-item {
    flex: 1;
  }
}
```

### 4. スクロール動作

#### デスクトップでのスクロール制御
- 左カラム（フォーム）: 独立スクロール
- 右カラム（テーブル）: 独立スクロール
- Header/Footer: 常に表示

```css
@media (min-width: 1024px) {
  /* 左カラムのスクロール */
  .left-column {
    overflow-y: auto;
    max-height: calc(100vh - 140px);
  }
  
  /* 右カラムのスクロール */
  .right-column {
    overflow-y: auto;
    max-height: calc(100vh - 140px);
  }
}
```

### 5. 余白・パディング調整

#### レスポンシブ余白
```css
/* モバイル */
ion-content {
  --padding-top: 16px;
  --padding-bottom: 16px;
  --padding-start: 12px;
  --padding-end: 12px;
}

/* タブレット */
@media (min-width: 768px) {
  ion-content {
    --padding-top: 20px;
    --padding-bottom: 20px;
    --padding-start: 16px;
    --padding-end: 16px;
  }
}

/* デスクトップ */
@media (min-width: 1024px) {
  ion-content {
    --padding-top: 24px;
    --padding-bottom: 24px;
    --padding-start: 20px;
    --padding-end: 20px;
  }
}
```

### 6. ナビゲーション対応

#### モバイル対応
```css
/* モバイル: ハンバーガーメニュー表示（将来） */
@media (max-width: 767px) {
  .menu-button {
    display: block;
  }
  
  .nav-menu {
    display: none;
  }
}

/* デスクトップ: 通常メニュー */
@media (min-width: 1024px) {
  .menu-button {
    display: none;
  }
  
  .nav-menu {
    display: flex;
  }
}
```

## Implementation Strategy

### Phase 1: CSS 設計
1. メディアクエリの定義
2. グリッドレイアウトの実装
3. 既存スタイルのリセット・調整

### Phase 2: HTML 構造の調整
1. セクションの分割（左カラム・右カラム）
2. ion-grid/ion-col の活用（または CSS Grid）
3. スクロール可能な要素の明示

### Phase 3: テスト・調整
1. 各ブレークポイントでの表示確認
2. スクロール動作の検証
3. 既存機能の動作確認

## File Changes

### Modified Files
- `src/web/templates/index.html`
  - CSS: メディアクエリ追加、グリッドレイアウト
  - HTML: セクション構造の調整

## Responsive Breakpoints

| Device | Width | Layout | Columns |
|--------|-------|--------|---------|
| Mobile | <768px | Stack | 1 |
| Tablet | 768-1023px | Stack/Side-by-side | 1-2 |
| Desktop | ≥1024px | Side-by-side | 2 |

## Browser Support

- Chrome/Edge 90+
- Firefox 88+
- Safari 14+
- iOS Safari 14+
- Android Chrome 90+

## Performance Considerations

1. **CSS Grid/Flexbox**: ネイティブ実装で高速
2. **メディアクエリ**: ビルド時に最適化可能
3. **スクロール**: `contain: layout` で最適化
4. **Repaint 最小化**: ブレークポイント間で最小限の変更

## Accessibility

1. キーボード操作: 変わらず対応
2. スクリーンリーダー: レイアウト変更の影響なし
3. タッチターゲット: モバイルで 44px 以上確保
4. コントラスト: 現在のまま維持

## Future Extensions

1. **設定による左右入れ替え**: 将来的に実装可能な構造設計
2. **ダークモード対応**: 既存の CSS 変数活用
3. **カスタマイズ可能なカラム幅**: CSS 変数で実装可能

## Testing Strategy

### Unit Tests
- メディアクエリのブレークポイント確認
- スクロール動作の検証

### Integration Tests
- すべてのブレークポイントでの UI テスト
- 既存機能（投稿、編集、削除）の動作確認
- クロスブラウザテスト

### Manual Testing
- デバイス（モバイル、タブレット、デスクトップ）での確認
- レスポンシブデザインモードでの検証
