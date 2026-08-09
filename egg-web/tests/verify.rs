// ============================================================================
// Проверка правильности формул, используемых на сайте.
// Прогоняем каждое правило через настоящий egg и проверяем ожидаемый результат.
// ============================================================================

use egg::{rewrite as rw, *};

fn all_rules() -> Vec<Rewrite<SymbolLang, ()>> {
    vec![
        rw!("commute-add"; "(+ ?x ?y)" => "(+ ?y ?x)"),
        rw!("commute-mul"; "(* ?x ?y)" => "(* ?y ?x)"),
        rw!("add-0"; "(+ ?x 0)" => "?x"),
        rw!("mul-0"; "(* ?x 0)" => "0"),
        rw!("mul-1"; "(* ?x 1)" => "?x"),
        rw!("add-same"; "(+ ?x ?x)" => "(* 2 ?x)"),
        rw!("mul-assoc"; "(* (* ?x ?y) ?z)" => "(* ?x (* ?y ?z))"),
    ]
}

/// Складывает оба выражения в e-graph и возвращает true, если они в одном классе.
fn same_class(egraph: &EGraph<SymbolLang, ()>, a: &str, b: &str) -> bool {
    let ea: RecExpr<SymbolLang> = a.parse().unwrap();
    let eb: RecExpr<SymbolLang> = b.parse().unwrap();
    match (egraph.lookup_expr(&ea), egraph.lookup_expr(&eb)) {
        (Some(x), Some(y)) => egraph.find(x) == egraph.find(y),
        _ => false,
    }
}

fn run(expr: &str, rules: &[Rewrite<SymbolLang, ()>]) -> Runner<SymbolLang, ()> {
    let start: RecExpr<SymbolLang> = expr.parse().unwrap();
    Runner::default().with_expr(&start).run(rules)
}

#[test]
fn add_0_neutral() {
    let eg = run("(+ a 0)", &[rw!("add-0"; "(+ ?x 0)" => "?x")]);
    assert!(same_class(&eg.egraph, "(+ a 0)", "a"), "x + 0 должно быть равно x");
}

#[test]
fn mul_0_zero() {
    let eg = run("(* a 0)", &[rw!("mul-0"; "(* ?x 0)" => "0")]);
    assert!(same_class(&eg.egraph, "(* a 0)", "0"), "x * 0 должно быть равно 0");
}

#[test]
fn mul_1_neutral() {
    let eg = run("(* a 1)", &[rw!("mul-1"; "(* ?x 1)" => "?x")]);
    assert!(same_class(&eg.egraph, "(* a 1)", "a"), "x * 1 должно быть равно x");
}

#[test]
fn commute_add() {
    let eg = run("(+ a b)", &[rw!("commute-add"; "(+ ?x ?y)" => "(+ ?y ?x)")]);
    assert!(same_class(&eg.egraph, "(+ a b)", "(+ b a)"));
}

#[test]
fn commute_mul() {
    let eg = run("(* a b)", &[rw!("commute-mul"; "(* ?x ?y)" => "(* ?y ?x)")]);
    assert!(same_class(&eg.egraph, "(* a b)", "(* b a)"));
}

#[test]
fn add_same_doubles() {
    let eg = run("(+ a a)", &[rw!("add-same"; "(+ ?x ?x)" => "(* 2 ?x)")]);
    assert!(same_class(&eg.egraph, "(+ a a)", "(* 2 a)"));
}

#[test]
fn mul_assoc() {
    let eg = run("(* (* a b) c)", &[rw!("mul-assoc"; "(* (* ?x ?y) ?z)" => "(* ?x (* ?y ?z))")]);
    assert!(same_class(&eg.egraph, "(* (* a b) c)", "(* a (* b c))"));
}

/// Демо-выражение с полным набором правил: проверяем, что извлекается
/// выражение стоимости 5 и что (+ b b) сливается с (* 2 b).
#[test]
fn demo_full_saturation() {
    let rules = all_rules();
    let start: RecExpr<SymbolLang> = "(* (+ 0 (* 1 a)) (+ b b))".parse().unwrap();
    let runner = Runner::default().with_expr(&start).run(&rules);

    assert_eq!(runner.iterations.len(), 7, "ожидалось 7 итераций до насыщения");
    assert!(same_class(&runner.egraph, "(+ b b)", "(* 2 b)"));
    assert!(same_class(&runner.egraph, "(+ 0 (* 1 a))", "a"));

    let extractor = Extractor::new(&runner.egraph, AstSize);
    let (cost, best) = extractor.find_best(runner.roots[0]);
    assert_eq!(cost, 5, "минимальная стоимость должна быть 5");
    let valid = [
        "(* a (+ b b))",
        "(* a (* 2 b))",
        "(* (+ b b) a)",
        "(* (* 2 b) a)",
        "(* a (* b 2))",
        "(* (* b 2) a)",
    ];
    assert!(
        valid.contains(&best.to_string().as_str()),
        "неожиданный результат извлечения: {best}"
    );
}

/// Без правил граф не растёт, извлекается исходное выражение.
/// (Runner делает ровно 1 итерацию поиска: не находит совпадений и останавливается.)
#[test]
fn no_rules_no_growth() {
    let start: RecExpr<SymbolLang> = "(* (+ 0 (* 1 a)) (+ b b))".parse().unwrap();
    let runner = Runner::default().with_expr(&start).run(&[] as &[Rewrite<SymbolLang, ()>]);
    assert_eq!(runner.iterations.len(), 1, "ожидалась одна пустая итерация поиска");

    let mut bare: EGraph<SymbolLang, ()> = Default::default();
    bare.add_expr(&start);
    bare.rebuild();
    assert_eq!(
        runner.egraph.number_of_classes(),
        bare.number_of_classes(),
        "без правил граф не должен расти"
    );

    let extractor = Extractor::new(&runner.egraph, AstSize);
    let (_cost, best) = extractor.find_best(runner.roots[0]);
    assert_eq!(best.to_string(), "(* (+ 0 (* 1 a)) (+ b b))");
}

/// Граф растёт с итерациями (логика ползунка на сайте).
#[test]
fn graph_grows_with_iterations() {
    let rules = all_rules();
    let start: RecExpr<SymbolLang> = "(* (+ 0 (* 1 a)) (+ b b))".parse().unwrap();

    let early = Runner::default().with_expr(&start).with_iter_limit(2).run(&rules);
    let full = Runner::default().with_expr(&start).run(&rules);

    assert!(early.egraph.number_of_classes() < full.egraph.number_of_classes());
    assert!(early.egraph.total_number_of_nodes() < full.egraph.total_number_of_nodes());
}

/// Часть 1 сайта: конгруэнтное замыкание f(a,b) и f(a,c).
#[test]
fn congruence_closure() {
    let mut eg: EGraph<SymbolLang, ()> = Default::default();
    let a = eg.add(SymbolLang::leaf("a"));
    let b = eg.add(SymbolLang::leaf("b"));
    let c = eg.add(SymbolLang::leaf("c"));
    let fab = eg.add(SymbolLang::new("f", vec![a, b]));
    let fac = eg.add(SymbolLang::new("f", vec![a, c]));
    eg.rebuild();
    assert_ne!(eg.find(fab), eg.find(fac), "до равенств классы должны различаться");

    eg.union(a, b);
    eg.union(b, c);
    eg.rebuild();
    assert_eq!(eg.find(fab), eg.find(fac), "после a=b, b=c классы должны слиться");
    assert_eq!(eg.number_of_classes(), 2, "должно остаться 2 класса");
}
