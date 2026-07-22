# Requirements Document

## Project Description (Input)
ダークモードの実装と切り替え機能(+システム連動)

## Requirements
<!-- Will be generated in /kiro:spec-requirements phase -->

## 現状 (2026-07-22 棚卸し)

`src/web/templates/index.html` にて `prefers-color-scheme` によるシステム連動は実装済み。
手動切替UI(`data-theme` 属性 + localStorage 永続化)は未実装。

関連Issue: #58
