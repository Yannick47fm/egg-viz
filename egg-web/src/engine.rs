// ============================================================================
// engine: чистый Rust-модуль (не зависит от web-sys/wasm) — то, что можно
// покрывать обычными тестами на хосте:
//   * разбор выражений и правил: инфиксная запись или s-выражения egg;
//   * сборка Rewrite<SymbolLang> из строки;
//   * наивный терм-райтер для сравнения с e-graph'ом.
// ============================================================================

use std::collections::HashMap;
use std::fmt::Write;

use egg::{Id, Pattern, RecExpr, Rewrite, SymbolLang};

// --- токенизация -------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Atom(String),
    Op(String),
    LParen,
    RParen,
}

fn tokenize(s: &str) -> Result<Vec<Tok>, String> {
    let mut out = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\t' | '\n' | '\r' => {
                chars.next();
            }
            '(' => {
                chars.next();
                out.push(Tok::LParen);
            }
            ')' => {
                chars.next();
                out.push(Tok::RParen);
            }
            '+' | '-' | '*' | '/' => {
                chars.next();
                out.push(Tok::Op(c.to_string()));
            }
            c if c.is_alphanumeric() || c == '_' || c == '?' => {
                let mut atom = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' || c == '?' {
                        atom.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                out.push(Tok::Atom(atom));
            }
            _ => return Err(format!("неожиданный символ '{c}'")),
        }
    }
    Ok(out)
}

// --- дерево выражений ---------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tree {
    Leaf(String),
    Node(String, Vec<Tree>),
}

impl Tree {
    fn leaf(s: &str) -> Self {
        Tree::Leaf(s.to_string())
    }
    fn node(op: &str, kids: Vec<Tree>) -> Self {
        Tree::Node(op.to_string(), kids)
    }
}

/// Канонический вид дерева — s-выражение.
pub fn tree_to_sexpr(t: &Tree) -> String {
    match t {
        Tree::Leaf(s) => s.clone(),
        Tree::Node(op, kids) => {
            let mut s = format!("({op}");
            for k in kids {
                write!(s, " {}", tree_to_sexpr(k)).unwrap();
            }
            s.push(')');
            s
        }
    }
}

/// Дерево в RecExpr egg (для построения e-graph'а).
pub fn tree_to_recexpr(t: &Tree) -> RecExpr<SymbolLang> {
    fn add(r: &mut RecExpr<SymbolLang>, t: &Tree) -> Id {
        match t {
            Tree::Leaf(s) => r.add(SymbolLang::leaf(s)),
            Tree::Node(op, kids) => {
                let ids: Vec<Id> = kids.iter().map(|k| add(r, k)).collect();
                r.add(SymbolLang::new(op, ids))
            }
        }
    }
    let mut r = RecExpr::default();
    add(&mut r, t);
    r
}

// --- разбор ------------------------------------------------------------------

fn parse_sexpr(toks: &[Tok], pos: &mut usize) -> Result<Tree, String> {
    match toks.get(*pos) {
        Some(Tok::LParen) => {
            *pos += 1;
            let mut kids = Vec::new();
            while toks.get(*pos) != Some(&Tok::RParen) {
                if toks.get(*pos).is_none() {
                    return Err("незакрытая скобка".to_string());
                }
                kids.push(parse_sexpr(toks, pos)?);
            }
            *pos += 1;
            if kids.is_empty() {
                return Err("пустые скобки ()".to_string());
            }
            match kids.remove(0) {
                Tree::Leaf(op) => Ok(Tree::node(&op, kids)),
                Tree::Node(..) => Err("первый элемент списка должен быть операцией".to_string()),
            }
        }
        Some(Tok::Atom(s)) => {
            *pos += 1;
            Ok(Tree::leaf(s))
        }
        Some(Tok::Op(s)) => {
            *pos += 1;
            Ok(Tree::leaf(s))
        }
        _ => Err("ожидался атом или список".to_string()),
    }
}

fn parse_infix_expr(toks: &[Tok], pos: &mut usize) -> Result<Tree, String> {
    let mut left = parse_infix_term(toks, pos)?;
    loop {
        match toks.get(*pos) {
            Some(Tok::Op(s)) if s == "+" || s == "-" => {
                *pos += 1;
                let right = parse_infix_term(toks, pos)?;
                left = Tree::node(s, vec![left, right]);
            }
            _ => return Ok(left),
        }
    }
}

fn parse_infix_term(toks: &[Tok], pos: &mut usize) -> Result<Tree, String> {
    let mut left = parse_infix_factor(toks, pos)?;
    loop {
        match toks.get(*pos) {
            Some(Tok::Op(s)) if s == "*" || s == "/" => {
                *pos += 1;
                let right = parse_infix_factor(toks, pos)?;
                left = Tree::node(s, vec![left, right]);
            }
            _ => return Ok(left),
        }
    }
}

fn parse_infix_factor(toks: &[Tok], pos: &mut usize) -> Result<Tree, String> {
    match toks.get(*pos) {
        Some(Tok::Atom(s)) => {
            *pos += 1;
            Ok(Tree::leaf(s))
        }
        Some(Tok::LParen) => {
            *pos += 1;
            let t = parse_infix_expr(toks, pos)?;
            if toks.get(*pos) != Some(&Tok::RParen) {
                return Err("незакрытая скобка".to_string());
            }
            *pos += 1;
            Ok(t)
        }
        _ => Err("ожидался атом или скобка".to_string()),
    }
}

/// Разбирает выражение или паттерн: сначала как инфиксную запись
/// (a * 2 + b), затем как s-выражение egg ((* a 2) (+ ...)).
pub fn parse_any(s: &str) -> Result<Tree, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("пустой ввод".to_string());
    }
    let toks = tokenize(s)?;

    let mut pos = 0;
    if let Ok(t) = parse_infix_expr(&toks, &mut pos) {
        if pos == toks.len() {
            return Ok(t);
        }
    }
    let mut pos = 0;
    if let Ok(t) = parse_sexpr(&toks, &mut pos) {
        if pos == toks.len() {
            return Ok(t);
        }
    }
    Err(format!(
        "не смог разобрать «{s}» ни как инфиксное, ни как s-выражение"
    ))
}

/// Выражение → каноническое s-выражение (для тестов и отображения).
pub fn parse_expr_str(s: &str) -> Result<String, String> {
    parse_any(s).map(|t| tree_to_sexpr(&t))
}

// --- правила -----------------------------------------------------------------

/// Разбирает строку правила: «имя: lhs => rhs».
/// lhs/rhs — инфикс или s-запись, ?x — переменные.
/// Имя необязательно: без него будет custom-N.
pub fn parse_rule_str(line: &str, idx: usize) -> Result<(String, String, String), String> {
    let mut name = format!("custom-{idx}");
    let mut rest = line;
    if let Some(colon) = line.find(':') {
        let maybe = line[..colon].trim();
        if !maybe.is_empty()
            && maybe
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            name = maybe.to_string();
            rest = &line[colon + 1..];
        }
    }
    let arrow = rest.find("=>").ok_or_else(|| {
        format!("в правиле «{line}» нет стрелки «=>» (пример: idemp: (+ ?x 0) => ?x)")
    })?;
    let lhs = rest[..arrow].trim();
    let rhs = rest[arrow + 2..].trim();
    if lhs.is_empty() || rhs.is_empty() {
        return Err(format!("пустая сторона в правиле «{line}»"));
    }
    let lhs_s = parse_expr_str(lhs)?;
    let rhs_s = parse_expr_str(rhs)?;
    Ok((name, lhs_s, rhs_s))
}

/// Собирает Rewrite egg'а из строк левой и правой части (s-запись).
pub fn make_rule(name: &str, lhs: &str, rhs: &str) -> Result<Rewrite<SymbolLang, ()>, String> {
    let searcher: Pattern<SymbolLang> = lhs
        .parse()
        .map_err(|e| format!("левая часть «{lhs}»: {e:?}"))?;
    let applier: Pattern<SymbolLang> = rhs
        .parse()
        .map_err(|e| format!("правая часть «{rhs}»: {e:?}"))?;
    Rewrite::new(name.to_string(), searcher, applier)
        .map_err(|e| format!("правило «{name}»: {e}"))
}

// --- наивный терм-райтер ------------------------------------------------------

fn match_tree(pat: &Tree, subj: &Tree, subst: &mut HashMap<String, Tree>) -> bool {
    match (pat, subj) {
        (Tree::Leaf(v), _) if v.starts_with('?') => match subst.get(v) {
            Some(prev) => prev == subj,
            None => {
                subst.insert(v.clone(), subj.clone());
                true
            }
        },
        (Tree::Leaf(a), Tree::Leaf(b)) => a == b,
        (Tree::Node(op1, k1), Tree::Node(op2, k2))
            if op1 == op2 && k1.len() == k2.len() =>
        {
            let mut ok = true;
            for (p, s) in k1.iter().zip(k2.iter()) {
                ok &= match_tree(p, s, subst);
            }
            ok
        }
        _ => false,
    }
}

fn instantiate(pat: &Tree, subst: &HashMap<String, Tree>) -> Tree {
    match pat {
        Tree::Leaf(v) if v.starts_with('?') => subst
            .get(v)
            .cloned()
            .unwrap_or_else(|| Tree::leaf(v)),
        Tree::Leaf(s) => Tree::leaf(s),
        Tree::Node(op, kids) => {
            Tree::node(op, kids.iter().map(|k| instantiate(k, subst)).collect())
        }
    }
}

/// Один проход сверху вниз: первое же совпадение переписывается.
fn apply_once(t: &Tree, rules: &[(String, Tree, Tree)]) -> Option<Tree> {
    for (_, lhs, rhs) in rules {
        let mut subst = HashMap::new();
        if match_tree(lhs, t, &mut subst) {
            return Some(instantiate(rhs, &subst));
        }
    }
    if let Tree::Node(op, kids) = t {
        let mut new_kids = kids.clone();
        for k in new_kids.iter_mut() {
            if let Some(nk) = apply_once(k, rules) {
                *k = nk;
                return Some(Tree::node(op, new_kids));
            }
        }
    }
    None
}

#[derive(Clone, Debug, Default)]
pub struct NaiveOut {
    pub ok: bool,
    pub error: String,
    pub converged: bool,
    pub steps: usize,
    pub best: String,
}

/// Наивное жадное переписывание (для сравнения с e-graph'ом).
/// rules — строки правил (как в parse_rule_str), по одной на строку.
pub fn naive_rewrite(expr: &str, rules: &str) -> NaiveOut {
    let mut tree = match parse_any(expr) {
        Ok(t) => t,
        Err(e) => return NaiveOut { ok: false, error: e, ..Default::default() },
    };

    let mut parsed = Vec::new();
    let mut idx = 0usize;
    for line in rules.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        idx += 1;
        match parse_rule_str(line, idx) {
            Ok((name, lhs_s, rhs_s)) => {
                let lhs = match parse_any(&lhs_s) {
                    Ok(t) => t,
                    Err(e) => return NaiveOut { ok: false, error: e, ..Default::default() },
                };
                let rhs = match parse_any(&rhs_s) {
                    Ok(t) => t,
                    Err(e) => return NaiveOut { ok: false, error: e, ..Default::default() },
                };
                parsed.push((name, lhs, rhs));
            }
            Err(e) => return NaiveOut { ok: false, error: e, ..Default::default() },
        }
    }

    const CAP: usize = 2000;
    let mut steps = 0usize;
    loop {
        match apply_once(&tree, &parsed) {
            Some(nt) => {
                tree = nt;
                steps += 1;
                if steps >= CAP {
                    return NaiveOut {
                        ok: true,
                        converged: false,
                        steps,
                        best: tree_to_sexpr(&tree),
                        ..Default::default()
                    };
                }
            }
            None => {
                return NaiveOut {
                    ok: true,
                    converged: true,
                    steps,
                    best: tree_to_sexpr(&tree),
                    ..Default::default()
                };
            }
        }
    }
}
