// ============================================================================
// egg-web: интерактивная визуализация e-graph'ов в браузере.
//
// Настоящая библиотека egg (Rust) скомпилирована в WebAssembly и работает
// прямо на странице. UI — на web-sys (DOM), рендер графов — Graphviz WASM
// (viz.js), которому мы отдаём DOT-вывод egg'а через глобальную функцию
// renderDot (объявлена в index.html).
//
// Вся разборка/логика — в модуле engine (чистый Rust, покрыт тестами).
// ============================================================================

pub mod engine;

use egg::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{window, HtmlElement, HtmlInputElement, HtmlTextAreaElement};

#[wasm_bindgen]
extern "C" {
    // Глобальная функция из index.html: рендерит DOT в SVG внутри <div id=...>.
    #[wasm_bindgen(js_name = renderDot)]
    fn render_dot(dot: &str, container_id: &str);
}

// --- мелкие помощники для DOM -------------------------------------------------

fn el<T: JsCast>(id: &str) -> T {
    window()
        .expect("нет window")
        .document()
        .expect("нет document")
        .get_element_by_id(id)
        .unwrap_or_else(|| panic!("нет элемента #{id}"))
        .dyn_into::<T>()
        .unwrap()
}

fn set_text(id: &str, text: &str) {
    el::<HtmlElement>(id).set_text_content(Some(text));
}

fn add_click(id: &str, mut f: impl FnMut() + 'static) {
    let cb = Closure::wrap(Box::new(move || f()) as Box<dyn FnMut()>);
    el::<HtmlElement>(id)
        .add_event_listener_with_callback("click", cb.as_ref().unchecked_ref())
        .expect("не удалось повесить обработчик");
    cb.forget();
}

// --- правила и выражение ------------------------------------------------------

/// Встроенные правила: (id чекбокса, имя, левая часть, правая часть).
/// Строки одинаково используются и для egg, и для наивного райтера.
const BUILTIN_RULES: [(&str, &str, &str, &str); 7] = [
    ("rule-commute-add", "commute-add", "(+ ?x ?y)", "(+ ?y ?x)"),
    ("rule-commute-mul", "commute-mul", "(* ?x ?y)", "(* ?y ?x)"),
    ("rule-add-0", "add-0", "(+ ?x 0)", "?x"),
    ("rule-mul-0", "mul-0", "(* ?x 0)", "0"),
    ("rule-mul-1", "mul-1", "(* ?x 1)", "?x"),
    ("rule-add-same", "add-same", "(+ ?x ?x)", "(* 2 ?x)"),
    ("rule-mul-assoc", "mul-assoc", "(* (* ?x ?y) ?z)", "(* ?x (* ?y ?z))"),
];

/// Выбранные галочками + пользовательские правила из textarea.
/// Ошибка в любом правиле останавливает запуск с понятным сообщением.
fn selected_rules() -> Result<Vec<Rewrite<SymbolLang, ()>>, String> {
    let mut rules = Vec::new();
    for (id, name, lhs, rhs) in BUILTIN_RULES {
        if el::<HtmlInputElement>(id).checked() {
            rules.push(engine::make_rule(name, lhs, rhs)?);
        }
    }
    let custom = el::<HtmlTextAreaElement>("rules-custom").value();
    let mut idx = 0usize;
    for line in custom.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        idx += 1;
        let (name, lhs, rhs) = engine::parse_rule_str(line, idx)?;
        rules.push(engine::make_rule(&name, &lhs, &rhs)?);
    }
    Ok(rules)
}

/// Тексты правил для наивного райтера (встроенные выбранные + пользовательские).
fn rules_text() -> String {
    let mut s = String::new();
    for (id, name, lhs, rhs) in BUILTIN_RULES {
        if el::<HtmlInputElement>(id).checked() {
            s.push_str(&format!("{name}: {lhs} => {rhs}\n"));
        }
    }
    s.push_str(&el::<HtmlTextAreaElement>("rules-custom").value());
    s
}

fn parse_expr() -> Option<RecExpr<SymbolLang>> {
    let text = el::<HtmlTextAreaElement>("expr-input").value();
    match engine::parse_any(&text).map(|t| engine::tree_to_recexpr(&t)) {
        Ok(expr) => {
            set_text("status", "");
            Some(expr)
        }
        Err(e) => {
            set_text("status", &e);
            None
        }
    }
}

// --- раздел 1: equality saturation ----------------------------------------------

fn show_iteration(expr: &RecExpr<SymbolLang>, rules: &[Rewrite<SymbolLang, ()>], iter: usize) {
    // Запускаем сам egg (в браузере!) с ограничением на число итераций.
    let runner = Runner::default().with_expr(expr).with_iter_limit(iter).run(rules);
    let egraph = &runner.egraph;

    render_dot(&egraph.dot().to_string(), "svg-box");

    set_text(
        "stats",
        &format!(
            "Итераций: {}, классов эквивалентности: {}, узлов: {}",
            runner.iterations.len(),
            egraph.number_of_classes(),
            egraph.total_number_of_nodes()
        ),
    );

    let extractor = Extractor::new(egraph, AstSize);
    let (cost, best) = extractor.find_best(runner.roots[0]);
    set_text("extract-result", &format!("{best}  (стоимость {cost})"));
}

fn run_full(expr: &RecExpr<SymbolLang>, rules: &[Rewrite<SymbolLang, ()>]) {
    let runner = Runner::default().with_expr(expr).run(rules);
    let max_iter = runner.iterations.len();
    let slider = el::<HtmlInputElement>("iter-slider");
    slider.set_max(&max_iter.to_string());
    slider.set_value(&max_iter.to_string());
    set_text("iter-label", &format!("{max_iter} из {max_iter}"));
    show_iteration(expr, rules, max_iter);
}

// --- раздел 2: почему не наивный райтер -------------------------------------------

fn run_naive() {
    let expr_text = el::<HtmlTextAreaElement>("expr-input").value();
    let out = engine::naive_rewrite(&expr_text, &rules_text());
    if !out.ok {
        set_text("naive-status", &out.error);
        return;
    }
    set_text(
        "naive-status",
        &if out.converged {
            format!("Сошёлся: {steps} шагов", steps = out.steps)
        } else {
            format!(
                "НЕ сошёлся за {steps} шагов (ограничение) — зациклился",
                steps = out.steps
            )
        },
    );
    set_text("naive-result", &out.best);
}

// --- раздел 3: конгруэнтное замыкание --------------------------------------------

fn toggle(id: &str) {
    let btn = el::<HtmlElement>(id);
    let active = btn.dataset().get("active").as_deref() == Some("1");
    let next = if active { "0" } else { "1" };
    let _ = btn.dataset().set("active", next);
    btn.set_class_name(if next == "1" { "toggle on" } else { "toggle" });
}

fn show_part1() {
    let mut eg: EGraph<SymbolLang, ()> = Default::default();
    let a = eg.add(SymbolLang::leaf("a"));
    let b = eg.add(SymbolLang::leaf("b"));
    let c = eg.add(SymbolLang::leaf("c"));
    let fab = eg.add(SymbolLang::new("f", vec![a, b]));
    let fac = eg.add(SymbolLang::new("f", vec![a, c]));

    if el::<HtmlElement>("btn-ab").dataset().get("active").as_deref() == Some("1") {
        eg.union(a, b);
    }
    if el::<HtmlElement>("btn-bc").dataset().get("active").as_deref() == Some("1") {
        eg.union(b, c);
    }
    eg.rebuild();

    render_dot(&eg.dot().to_string(), "svg-box2");

    let same = eg.find(fab) == eg.find(fac);
    set_text(
        "part1-status",
        &format!(
            "f(a,b) и f(a,c) в одном классе: {} | всего классов: {}",
            if same { "ДА" } else { "нет" },
            eg.number_of_classes()
        ),
    );
}

// --- точка входа -----------------------------------------------------------------

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();

    add_click("run-btn", || {
        if let Some(expr) = parse_expr() {
            match selected_rules() {
                Ok(rules) => run_full(&expr, &rules),
                Err(e) => set_text("status", &format!("Правила: {e}")),
            }
        }
    });

    {
        let cb = Closure::wrap(Box::new(|| {
            if let Some(expr) = parse_expr() {
                match selected_rules() {
                    Ok(rules) => {
                        let iter: usize = el::<HtmlInputElement>("iter-slider")
                            .value()
                            .parse()
                            .unwrap_or(0);
                        set_text("iter-label", &iter.to_string());
                        show_iteration(&expr, &rules, iter);
                    }
                    Err(e) => set_text("status", &format!("Правила: {e}")),
                }
            }
        }) as Box<dyn FnMut()>);
        el::<HtmlInputElement>("iter-slider")
            .add_event_listener_with_callback("input", cb.as_ref().unchecked_ref())
            .expect("не удалось повесить обработчик ползунка");
        cb.forget();
    }

    add_click("naive-btn", run_naive);

    add_click("btn-ab", || {
        toggle("btn-ab");
        show_part1();
    });
    add_click("btn-bc", || {
        toggle("btn-bc");
        show_part1();
    });

    // Стартовое состояние: сразу запускаем демо, чтобы страница не была пустой.
    if let Some(expr) = parse_expr() {
        match selected_rules() {
            Ok(rules) => run_full(&expr, &rules),
            Err(e) => set_text("status", &format!("Правила: {e}")),
        }
    }
    run_naive();
    show_part1();
}
