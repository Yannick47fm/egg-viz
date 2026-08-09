# egg-web

Интерактивный сайт, объясняющий работу библиотеки [egg](https://github.com/egraphs-good/egg)
(e-graphs / equality saturation). Сама библиотека egg скомпилирована в **WebAssembly**
и работает прямо в браузере.

## Что на сайте

1. **Equality saturation в действии** — ввод выражения (префиксная запись),
   выбор правил переписывания, ползунок по итерациям. Видно, как e-graph
   растёт от итерации к итерации и как из него извлекается минимальное
   выражение (`Extractor` + `AstSize`).
2. **Конгруэнтное замыкание** — кнопки «a = b» и «b = c»: объявляете равенства
   и видите, как `f(a,b)` и `f(a,c)` сливаются в один класс эквивалентности.
3. Текстовое объяснение того, что происходит.

Рендер графов — Graphviz, собранный в WASM ([viz.js](https://github.com/rhysd/viz-js),
загружается с CDN; для работы нужен интернет).

## Стек

- Rust + [egg](https://crates.io/crates/egg) (фича `wasm-bindgen`) → WASM
- [trunk](https://trunkrs.dev/) — сборка и раздача
- [web-sys](https://crates.io/crates/web-sys) — DOM из Rust, без JS-фреймворков

## Запуск

```sh
# в папке egg-web
trunk serve --release
# открыть http://127.0.0.1:8080
```

Продакшен-сборка: `trunk build --release` → статика в `dist/`
(можно залить на любой хостинг).

## Структура

- `index.html` — разметка, стили, инициализация viz.js и мост `window.renderDot`
- `src/lib.rs` — вся логика на Rust: парсинг выражения, `Runner` (egg),
  снапшоты графа, `Extractor`, обработчики UI
