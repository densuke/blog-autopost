/*
 * テーマ共通スクリプト (ダッシュボード / ログイン画面で共有)
 *
 * このファイルは <head> の先頭で同期読み込みされることを前提としている。
 * 同期スクリプトはパーサをブロックするため、body の解析も最初の描画も
 * 始まっていない時点で data-theme が確定する。よってちらつき (FOUC) は起きない。
 */
(function () {
    var KEY = 'blog-autopost-theme';

    /**
     * localStorage に保存された選択を読み出し、html 要素へ反映する。
     * 保存値が無い/不正な場合は 'auto' (システム設定連動) として扱う。
     */
    function applyStoredTheme() {
        var pref = null;
        try { pref = localStorage.getItem(KEY); } catch (e) { pref = null; }
        if (pref !== 'light' && pref !== 'dark') { pref = 'auto'; }
        var dark = pref === 'dark' || (pref === 'auto' &&
            window.matchMedia('(prefers-color-scheme: dark)').matches);
        var root = document.documentElement;
        root.setAttribute('data-theme-pref', pref);
        root.setAttribute('data-theme', dark ? 'dark' : 'light');
    }

    /**
     * slate 系の色を CSS 変数経由にして、テーマ毎に明暗を入れ替えられるようにする。
     * tailwind オブジェクトは CDN 読み込み後にしか存在しないため、
     * 各ページの CDN スクリプトより後で呼び出すこと。
     */
    function applyTailwindSlatePalette() {
        tailwind.config = {
            theme: {
                extend: {
                    colors: {
                        slate: [50, 100, 200, 300, 400, 500, 600, 700, 800, 900, 950]
                            .reduce(function (acc, key) {
                                acc[key] = 'rgb(var(--c-slate-' + key + ') / <alpha-value>)';
                                return acc;
                            }, {})
                    }
                }
            }
        };
    }

    window.THEME_STORAGE_KEY = KEY;
    window.applyStoredTheme = applyStoredTheme;
    window.applyTailwindSlatePalette = applyTailwindSlatePalette;

    // 描画前にテーマを確定させる
    applyStoredTheme();
})();
