# egg-viz: e-graphs и equality saturation — демо и интерактивный сайт

> **TL;DR (EN):** An interactive, Russian-language introduction to
> [e-graphs](https://egraphs-good.github.io/) and equality saturation built on the
> [egg](https://github.com/egraphs-good/egg) library. Two parts: a CLI demo that
> renders the e-graph with Graphviz, and a website where **real egg runs in the
> browser via WebAssembly** — type an expression (infix or s-expression), toggle
> rewrite rules or add your own, scrub through saturation iterations, and compare
> with a naive term rewriter to see why e-graphs never lose equivalent forms.

Проект для знакомства с библиотекой [egg](https://github.com/egraphs-good/egg) —
реализацией **e-graph'ов** (эквивалентностных графов) и алгоритма
**equality saturation** на Rust.

В репозитории две части:

| Часть | Описание |
|---|---|
| [`egg-demo/`](egg-demo) | Консольное демо: конгруэнтное замыкание + упрощение выражений правилами, с рендером e-graph в PNG/SVG (Graphviz) |
| [`egg-web/`](egg-web) | Интерактивный сайт: настоящий egg, скомпилированный в WebAssembly, работает прямо в браузере — живой e-graph, ползунок по итерациям, правила на галочках |

## Что такое egg

**E-graph** — структура данных, которая хранит выражение не как одно дерево (AST),
а как граф всех *эквивалентных* представлений:

- одинаковые подвыражения представлены одним узлом (класс эквивалентности, e-class);
- правила переписывания применяются не как в обычном терм-райтере (применить =
  заменить и **выбросить** старое), а как добавление нового равенства в граф —
  ничего не теряется;
- процесс применения правил до «насыщения» называется **equality saturation**,
  в конце из графа извлекается лучшее выражение по заданной стоимости
  (например, минимальный размер дерева).

Главный приём — **конгруэнтное замыкание**: если `a = b` и `b = c`, то
`f(a,b) = f(a,c)` для любого контекста `f`. Именно оно позволяет правилам
срабатывать внутри подвыражений.

## Быстрый старт

Требуется Rust (stable, 1.97+).

### CLI-демо

```sh
cd egg-demo
cargo run
```

Соберёт e-graph для `(* (+ 0 (* 1 a)) (+ b b))`, применит 7 правил до насыщения
и сохранит снимки графа в `egg-demo/viz/` (`.dot`, `.svg`, `.png`).
Для рендера нужен Graphviz (`dot` в PATH или переменная `GRAPHVIZ_BIN`).

### Сайт (WASM)

```sh
cd egg-web
trunk serve --release     # затем открыть http://127.0.0.1:8080
```

Продакшен-сборка:

```sh
cd egg-web
trunk build --release     # статика в egg-web/dist/
```

### Тесты

```sh
cd egg-web
cargo test                # 25 тестов: каждое правило, парсер, наивный райтер, конгруэнтное замыкание
```

## Сайт

Интерактивная страница с четырьмя разделами:

1. **Equality saturation** — ввод выражения (инфиксная запись `a * 2 + b` или
   s-запись `(* a 2) (+ ...)`), выбор правил галочками, свои правила в редакторе
   (`имя: lhs => rhs`, переменные `?x`), ползунок по итерациям: видно, как e-graph
   растёт, ничего не теряя, и как `Extractor` достаёт минимальное выражение
   (`AstSize`).
2. **Почему не просто терм-райтер?** — те же правила прогоняются через наивный
   жадный райтер: он застревает в локальном оптимуме или зацикливается, тогда как
   e-graph насыщается за конечное число итераций. Добавьте обратное правило
   `?x => (+ ?x 0)` — и сравните.
3. **Конгруэнтное замыкание** — кнопки «a = b», «b = c»: вживую видно слияние
   классов `f(a,b)` и `f(a,c)`.
4. Текстовое объяснение.

Стек: Rust + egg (фича `wasm-bindgen`) → [trunk](https://trunkrs.dev/) →
[web-sys](https://crates.io/crates/web-sys). Рендер графов — Graphviz в WASM
([viz.js](https://github.com/rhysd/viz-js)), `viz-standalone.js` встраивается
в сборку — внешних зависимостей нет. Разбор выражений и наивный райтер —
чистый Rust в `egg-web/src/engine.rs` (покрыт тестами, не зависит от wasm).

Онлайн-версия: **https://yannick47fm.github.io/egg-viz/** (GitHub Pages,
собирается через [GitHub Actions](.github/workflows/pages.yml)).

### Однофайловая версия

`egg-web/egg-web-onefile.html` — сайт одним файлом: WASM-модуль egg, JS-загрузчик
и Graphviz встроены внутрь, внешних зависимостей нет, работает офлайн.
Перегенерировать:

```sh
cd egg-web
trunk build --release
powershell -File ../scripts/build-onefile.ps1
```

## Правила демо

| Правило | Формула |
|---|---|
| commute-add | `(+ ?x ?y) => (+ ?y ?x)` |
| commute-mul | `(* ?x ?y) => (* ?y ?x)` |
| add-0 | `(+ ?x 0) => ?x` |
| mul-0 | `(* ?x 0) => 0` |
| mul-1 | `(* ?x 1) => ?x` |
| add-same | `(+ ?x ?x) => (* 2 ?x)` |
| mul-assoc | `(* (* ?x ?y) ?z) => (* ?x (* ?y ?z))` |

Любое правило можно выключить галочкой или добавить свои в редакторе на сайте.

## Структура репозитория

```
├── egg-demo/                 # CLI-демо (всё в одном src/main.rs)
│   ├── src/main.rs
│   └── README.md
├── egg-web/                  # сайт на WASM
│   ├── src/lib.rs            # UI на web-sys (обработчики, снапшоты, ползунок)
│   ├── src/engine.rs         # чистый Rust: парсер, свои правила, наивный райтер
│   ├── index.html            # разметка, стили, мост renderDot
│   ├── static/               # viz-standalone.js — Graphviz WASM, встраивается в сборку
│   ├── tests/verify.rs       # 25 тестов: правила, парсер, райтер
│   ├── egg-web-onefile.html  # сайт одним файлом
│   └── README.md
├── scripts/build-onefile.ps1 # генерация однофайловой версии
└── .github/workflows/pages.yml  # деплой на GitHub Pages
```

## Полезные ссылки

- Статья egg: [«egg: Fast and Extensible Equality Saturation», POPL 2021](https://dl.acm.org/doi/10.1145/3434304)
- [docs.rs/egg](https://docs.rs/egg) — API и туториалы
- [egraphs-good.github.io](https://egraphs-good.github.io/) — сайт проекта
- [egglog](https://github.com/egraphs-good/egglog) — Datalog-подход к e-graph'ам
- [awesome-egraphs](https://github.com/philzook58/awesome-egraphs) — список проектов на e-graph'ах

## Лицензия

MIT — см. [LICENSE](LICENSE).
