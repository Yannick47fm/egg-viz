# egg-viz: e-graphs и equality saturation — демо и интерактивный сайт

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
cargo test                # 11 тестов: каждое правило, извлечение, конгруэнтное замыкание
```

## Сайт

Интерактивная страница с тремя разделами:

1. **Equality saturation** — ввод выражения, выбор правил галочками, ползунок по
   итерациям: видно, как e-graph растёт, ничего не теряя, и как `Extractor`
   достаёт минимальное выражение (`AstSize`).
2. **Конгруэнтное замыкание** — кнопки «a = b», «b = c»: вживую видно слияние
   классов `f(a,b)` и `f(a,c)`.
3. Текстовое объяснение.

Стек: Rust + egg (фича `wasm-bindgen`) → [trunk](https://trunkrs.dev/) →
[web-sys](https://crates.io/crates/web-sys). Рендер графов — Graphviz в WASM
([viz.js](https://github.com/rhysd/viz-js)).

Онлайн-версия: **https://ybytor-byte.github.io/egg-viz/** (GitHub Pages,
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

## Структура репозитория

```
├── egg-demo/                 # CLI-демо (всё в одном src/main.rs)
│   ├── src/main.rs
│   └── README.md
├── egg-web/                  # сайт на WASM
│   ├── src/lib.rs            # вся логика на Rust (egg + web-sys)
│   ├── index.html            # разметка, стили, мост renderDot
│   ├── tests/verify.rs       # проверка формул через настоящий egg
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
