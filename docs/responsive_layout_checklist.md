# Responsive Layout Checklist

> **注意 (2026-07-22)**: このチェックリストの作業対象は
> `src/web/templates/index.html` であり、これは配信されない死にコードだった
> (PR #74 で削除済み)。実際に配信される `static/index.html` には
> 2カラムレイアウトが反映されていない。
>
> 履歴として残すが、現行のUIの状態を表すものではない。
> 現状と再実装の方針は `.kiro/specs/responsive-design-layout/` を参照。

- [x] Review existing web UI layout and identify adjustments needed for responsive two-column behavior
- [x] Implement template and style updates to support wider-screen two-column layout with independent scrolling panels
- [x] Verify visual structure (manual review or screenshots) and update checklist status
