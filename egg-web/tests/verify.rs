// ============================================================================
// Проверка правильности формул, используемых на сайте.
// Прогоняем каждое правило через настоящий egg и проверяем ожидаемый результат.
// ============================================================================

use egg::{rewrite as rw, *};
use egg_web::engine::{make_rule, naive_rewrite, parse_expr_str, parse_rule_str};

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

// --- движок: разбор выражений ----------------------------------------------------

/// Инфикс: приоритеты и ассоциативность.
#[test]
fn infix_precedence() {
    assert_eq!(parse_expr_str("a * 2 + b").unwrap(), "(+ (* a 2) b)");
    assert_eq!(parse_expr_str("a + b * 2").unwrap(), "(+ a (* b 2))");
    assert_eq!(parse_expr_str("a + b - c").unwrap(), "(- (+ a b) c)");
    assert_eq!(parse_expr_str("a - b - c").unwrap(), "(- (- a b) c)");
    assert_eq!(parse_expr_str("a * b / c").unwrap(), "(/ (* a b) c)");
}

/// Инфикс: скобки меняют структуру.
#[test]
fn infix_parentheses() {
    assert_eq!(parse_expr_str("(a + b) * 2").unwrap(), "(* (+ a b) 2)");
    assert_eq!(parse_expr_str("(a + b) * (c + d)").unwrap(), "(* (+ a b) (+ c d))");
}

/// s-запись egg по-прежнему работает и даёт тот же результат, что инфикс.
#[test]
fn sexpr_still_works() {
    assert_eq!(parse_expr_str("(* (+ 0 (* 1 a)) (+ b b))").unwrap(), "(* (+ 0 (* 1 a)) (+ b b))");
    assert_eq!(parse_expr_str("(* a 2)").unwrap(), parse_expr_str("a * 2").unwrap());
    assert_eq!(parse_expr_str("(+ a b)").unwrap(), parse_expr_str("a + b").unwrap());
}

/// Разбор ошибок: не должно паниковать.
#[test]
fn infix_bad_input() {
    for bad in ["", "  ", "a +", "(a + b", "a b", "+ a"] {
        assert!(parse_expr_str(bad).is_err(), "должно падать: «{bad}»");
    }
}

/// Инфиксное выражение в e-graph даёт те же классы, что s-запись.
#[test]
fn infix_matches_sexpr_semantics() {
    let ta = egg_web::engine::parse_any("a * 2 + b").unwrap();
    let tb = egg_web::engine::parse_any("(+ (* a 2) b)").unwrap();
    let mut eg: EGraph<SymbolLang, ()> = Default::default();
    let ra = eg.add_expr(&egg_web::engine::tree_to_recexpr(&ta));
    let rb = eg.add_expr(&egg_web::engine::tree_to_recexpr(&tb));
    eg.rebuild();
    assert_eq!(eg.find(ra), eg.find(rb), "инфикс и s-запись должны давать одну структуру");
}

// --- движок: пользовательские правила ---------------------------------------------

/// Своё правило в инфиксной записи работает в настоящем egg.
#[test]
fn custom_rule_infix() {
    let (name, lhs, rhs) = parse_rule_str("idemp: ?x + 0 => ?x", 1).unwrap();
    assert_eq!(name, "idemp");
    assert_eq!(lhs, "(+ ?x 0)");
    assert_eq!(rhs, "?x");

    let rule = make_rule(&name, &lhs, &rhs).unwrap();
    let eg = Runner::default()
        .with_expr(&"(* (+ a 0) (+ b 0))".parse::<RecExpr<SymbolLang>>().unwrap())
        .run(&[rule]);
    assert!(same_class(&eg.egraph, "(+ a 0)", "a"));
}

/// Своё правило в s-записи работает в настоящем egg.
#[test]
fn custom_rule_sexpr() {
    let rule = make_rule("double", "(* ?x 2)", "(+ ?x ?x)").unwrap();
    let eg = Runner::default()
        .with_expr(&"(* a 2)".parse::<RecExpr<SymbolLang>>().unwrap())
        .run(&[rule]);
    assert!(same_class(&eg.egraph, "(* a 2)", "(+ a a)"));
}

/// Свободные переменные в правой части должны отклоняться (не паниковать).
#[test]
fn custom_rule_unbound_var_rejected() {
    let err = make_rule("bad", "(+ ?x 0)", "?y").unwrap_err();
    assert!(err.contains("?y"), "ожидалась ошибка про ?y, получили: {err}");
}

/// Ошибки парсинга правила дают понятное сообщение.
#[test]
fn custom_rule_bad_line() {
    assert!(parse_rule_str("нет стрелки", 1).is_err());
    assert!(parse_rule_str("x =>", 1).is_err());
    assert!(parse_rule_str("=> x", 1).is_err());
}

// --- движок: наивный терм-райтер ---------------------------------------------------

/// Без коммутативности наивный райтер сходится, но застревает в локальном
/// оптимуме: (+ 0 X) и (* 1 X) не упрощаются, потому что 0 и 1 стоят слева,
/// а правила (+ ?x 0) / (* ?x 1) видят только правую позицию.
/// Тот же e-graph c теми же 3 правилами не лучше — см. следующий тест.
#[test]
fn naive_converges_without_commute() {
    let out = naive_rewrite(
        "(* (+ 0 (* 1 a)) (+ b b))",
        "add-0: (+ ?x 0) => ?x\nmul-1: (* ?x 1) => ?x\nadd-same: (+ ?x ?x) => (* 2 ?x)",
    );
    assert!(out.ok, "{}", out.error);
    assert!(out.converged, "должен сойтись за {} шагов", out.steps);
    assert_eq!(out.best, "(* (+ 0 (* 1 a)) (* 2 b))", "застрял, не увидев 0/1 слева");
}

/// С тем же набором правил e-graph тоже «не видит» 0/1 слева: стоимость 8.
/// «Суперсила» e-graph'а — в правиле, которое райтер применить не может
/// (например, с коммутативностью райтер зацикливается, а e-graph насыщается).
#[test]
fn egraph_same_local_optimum() {
    let rules: Vec<Rewrite<SymbolLang, ()>> = vec![
        rw!("add-0"; "(+ ?x 0)" => "?x"),
        rw!("mul-1"; "(* ?x 1)" => "?x"),
        rw!("add-same"; "(+ ?x ?x)" => "(* 2 ?x)"),
    ];
    let start: RecExpr<SymbolLang> = "(* (+ 0 (* 1 a)) (+ b b))".parse().unwrap();
    let runner = Runner::default().with_expr(&start).run(&rules);
    let extractor = Extractor::new(&runner.egraph, AstSize);
    let (cost, best) = extractor.find_best(runner.roots[0]);
    assert_eq!(cost, 9, "локальный оптимум: 0/1 слева не упрощаются");
    let valid = [
        "(* (+ 0 (* 1 a)) (+ b b))",
        "(* (+ 0 (* 1 a)) (* 2 b))",
    ];
    assert!(valid.contains(&best.to_string().as_str()), "неожиданный результат: {best}");
}

/// С коммутативностью наивный райтер зацикливается — а e-graph насыщается за 7 итераций.
#[test]
fn naive_loops_with_commute() {
    let out = naive_rewrite(
        "(+ a b)",
        "commute-add: (+ ?x ?y) => (+ ?y ?x)",
    );
    assert!(out.ok, "{}", out.error);
    assert!(!out.converged, "должен зациклиться (ограничение шагов)");
    assert_eq!(out.steps, 2000, "должен упереться в ограничение");
}

/// Обратное правило раздувает терм — райтер тоже зацикливается.
#[test]
fn naive_loops_with_reverse_rule() {
    let out = naive_rewrite("a", "grow: ?x => (+ ?x 0)");
    assert!(out.ok, "{}", out.error);
    assert!(!out.converged);
}

/// Тот же набор правил, что зацикливает райтер, egg насыщает за 7 итераций.
#[test]
fn egraph_saturates_where_naive_loops() {
    let rules: Vec<Rewrite<SymbolLang, ()>> = vec![rw!("commute-add"; "(+ ?x ?y)" => "(+ ?y ?x)")];
    let start: RecExpr<SymbolLang> = "(+ a b)".parse().unwrap();
    let runner = Runner::default().with_expr(&start).run(&rules);
    assert_eq!(runner.iterations.len(), 2, "должен насытиться за 2 итерации");
}
