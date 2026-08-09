// ============================================================================
// egg-web: интерактивная визуализация e-graph'ов в браузере.
//
// Настоящая библиотека egg (Rust) скомпилирована в WebAssembly и работает
// прямо на странице. UI — на web-sys (DOM), рендер графов — Graphviz WASM
// (viz.js), которому мы отдаём DOT-вывод egg'а через глобальную функцию
// renderDot (объявлена в index.html).
// ============================================================================

use egg::{rewrite as rw, *};
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

fn selected_rules() -> Vec<Rewrite<SymbolLang, ()>> {
    let all: [(&str, Rewrite<SymbolLang, ()>); 7] = [
        ("rule-commute-add", rw!("commute-add"; "(+ ?x ?y)" => "(+ ?y ?x)")),
        ("rule-commute-mul", rw!("commute-mul"; "(* ?x ?y)" => "(* ?y ?x)")),
        ("rule-add-0", rw!("add-0"; "(+ ?x 0)" => "?x")),
        ("rule-mul-0", rw!("mul-0"; "(* ?x 0)" => "0")),
        ("rule-mul-1", rw!("mul-1"; "(* ?x 1)" => "?x")),
        ("rule-add-same", rw!("add-same"; "(+ ?x ?x)" => "(* 2 ?x)")),
        ("rule-mul-assoc", rw!("mul-assoc"; "(* (* ?x ?y) ?z)" => "(* ?x (* ?y ?z))")),
    ];
    all.iter()
        .filter(|(id, _)| el::<HtmlInputElement>(id).checked())
        .map(|(_, rule)| rule.clone())
        .collect()
}

fn parse_expr() -> Option<RecExpr<SymbolLang>> {
    let text = el::<HtmlTextAreaElement>("expr-input").value();
    match text.parse() {
        Ok(expr) => {
            set_text("status", "");
            Some(expr)
        }
        Err(_) => {
            set_text("status", &format!("Не удалось разобрать выражение: {text}"));
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

// --- раздел 2: конгруэнтное замыкание --------------------------------------------

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
            let rules = selected_rules();
            run_full(&expr, &rules);
        }
    });

    {
        let cb = Closure::wrap(Box::new(|| {
            if let Some(expr) = parse_expr() {
                let rules = selected_rules();
                let iter: usize = el::<HtmlInputElement>("iter-slider")
                    .value()
                    .parse()
                    .unwrap_or(0);
                set_text("iter-label", &iter.to_string());
                show_iteration(&expr, &rules, iter);
            }
        }) as Box<dyn FnMut()>);
        el::<HtmlInputElement>("iter-slider")
            .add_event_listener_with_callback("input", cb.as_ref().unchecked_ref())
            .expect("не удалось повесить обработчик ползунка");
        cb.forget();
    }

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
        let rules = selected_rules();
        run_full(&expr, &rules);
    }
    show_part1();
}
