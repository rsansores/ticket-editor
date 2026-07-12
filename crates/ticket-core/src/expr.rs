//! The calculated-variable formula engine: a tiny, safe expression language.
//!
//! A `Computed` stores a formula string; this module parses it to an AST and
//! evaluates it against the variable data. It is intentionally minimal — a
//! spreadsheet-like expression, not a scripting language — and hardened the same
//! way the renderer is: bounded input length and nesting depth, capped array
//! iteration, no `unsafe`, and **no panics** on any input (parse errors surface
//! as `Err`, runtime type mismatches degrade to a blank value).
//!
//! Grammar (lowest-to-highest precedence):
//! ```text
//!   or      := and ( "or" and )*
//!   and     := cmp ( "and" cmp )*
//!   cmp     := add ( ("=="|"!="|"<"|"<="|">"|">=") add )?
//!   add     := mul ( ("+"|"-") mul )*
//!   mul     := unary ( ("*"|"/"|"%") unary )*
//!   unary   := ("-"|"not") unary | primary
//!   primary := number | string | "true" | "false"
//!            | ident "(" args ")"        // function call
//!            | ident                     // variable path (dotted, incl. array indexes)
//!            | "(" or ")"
//! ```
//! Variables are bare dotted paths (`sale.items.0.qty`, `calc.subtotal`). Text is
//! double-quoted (`"CASH"`, with `\"` and `\\` escapes). Aggregate functions take
//! a **list variable** as their first argument and evaluate the remaining
//! arguments once per row, with bare field names resolving against the current
//! row (`sumif(sale.movements, payment == "CASH", qty)`).

use serde_json::Value;

use std::cell::Cell;

use crate::data::{
    compare_strs, is_empty_value, number_value, resolve_scoped, to_number, value_to_string, Scope,
};

/// Reject absurd formulas outright (this is a per-variable string a human types).
const MAX_LEN: usize = 2000;
/// Cap parser/evaluator *recursion* depth (parenthesis / unary nesting), so a
/// pathologically nested formula can't overflow the stack while parsing.
const MAX_DEPTH: usize = 64;
/// Cap the total number of AST nodes a formula may build. This bounds a flat
/// operator chain (`a + b + c + …`) — which grows the tree but not the parser's
/// recursion — so evaluation never recurses deeper than the tree, and an
/// over-large formula fails to parse with a clear error instead of silently
/// evaluating to blank. Also the evaluator's recursion ceiling (a node count is
/// an upper bound on tree depth).
const MAX_NODES: usize = 1024;
/// Cap rows scanned by a single aggregate.
const MAX_ROWS: usize = 100_000;
/// Cap the total rows scanned across ALL aggregates in one formula, so nested
/// aggregates (`sumif(a, sumif(b, …) > 0, …)`) can't multiply into a stall.
const MAX_TOTAL_ROWS: u64 = 2_000_000;

/// Parse and evaluate `formula` against `root` in a scope (optional loop
/// binding plus optional `row.*` values) in one shot, with a fresh budget.
/// Production paths compile once and reuse ([`compile`] + [`eval_compiled`]);
/// this convenience remains for one-off evaluation and the test suite.
#[cfg_attr(not(test), allow(dead_code))]
pub fn eval_formula(formula: &str, root: &Value, scope: Scope) -> Result<Value, String> {
    let compiled = compile(formula)?;
    let budget = Cell::new(MAX_TOTAL_ROWS);
    Ok(eval_compiled(&compiled, root, scope, &budget))
}

/// A parsed, reusable formula. Row-scoped formulas evaluate once per loop
/// iteration — compiling once and evaluating the AST per row keeps a
/// 100k-item loop from re-parsing its formulas 100k times.
pub(crate) struct Compiled(Ast);

/// Parse a formula into a reusable [`Compiled`] AST.
pub(crate) fn compile(formula: &str) -> Result<Compiled, String> {
    if formula.len() > MAX_LEN {
        return Err(format!("formula too long (max {MAX_LEN} characters)"));
    }
    Ok(Compiled(parse(formula)?))
}

/// Evaluate a compiled formula, charging aggregate row scans to a
/// caller-provided budget. Sharing one budget across every formula of every
/// loop iteration is what bounds a whole render, not just a single formula —
/// a hostile document can't multiply per-formula budgets into a stall.
pub(crate) fn eval_compiled(
    compiled: &Compiled,
    root: &Value,
    scope: Scope,
    budget: &Cell<u64>,
) -> Value {
    eval(&compiled.0, root, scope, 0, budget)
}

/// The shared aggregate-row budget for one render or preview. Public to the
/// crate so `render`/`data` can allocate one per top-level operation.
pub(crate) fn fresh_budget() -> Cell<u64> {
    Cell::new(MAX_TOTAL_ROWS)
}

// ---------------------------------------------------------------------------
// AST
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Ast {
    Num(f64),
    Str(String),
    Bool(bool),
    Var(String),
    Unary(UnOp, Box<Ast>),
    Bin(BinOp, Box<Ast>, Box<Ast>),
    Call(Func, Vec<Ast>),
}

#[derive(Debug, Clone, Copy)]
enum UnOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Debug, Clone, Copy)]
enum Func {
    Concat,
    Round,
    Min,
    Max,
    Abs,
    Coalesce,
    Count,
    CountIf,
    Sum,
    SumIf,
    Avg,
    AvgIf,
}

impl Func {
    fn from_name(name: &str) -> Option<Func> {
        Some(match name {
            "concat" => Func::Concat,
            "round" => Func::Round,
            "min" => Func::Min,
            "max" => Func::Max,
            "abs" => Func::Abs,
            "coalesce" => Func::Coalesce,
            "count" => Func::Count,
            "countif" => Func::CountIf,
            "sum" => Func::Sum,
            "sumif" => Func::SumIf,
            "avg" => Func::Avg,
            "avgif" => Func::AvgIf,
            _ => return None,
        })
    }
}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Ident(String),
    Op(&'static str),
    LParen,
    RParen,
    Comma,
}

fn tokenize(src: &str) -> Result<Vec<Tok>, String> {
    // Iterate over CHARS, not bytes — indexing `src.as_bytes()` and slicing
    // `&src[i..i+2]` panics on a UTF-8 char boundary, and `byte as char`
    // mis-decodes multibyte text (mojibake). A ticket formula can contain
    // accented text or a `€`/emoji in a "string" literal, or a stray non-ASCII
    // char, so this must be char-safe (the crate forbids panics on any input).
    let mut toks = Vec::new();
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch.is_whitespace() {
            i += 1;
        } else if ch == '"' {
            // String literal with \" and \\ escapes.
            let mut s = String::new();
            i += 1;
            let mut closed = false;
            while i < chars.len() {
                let c = chars[i];
                if c == '\\' && i + 1 < chars.len() {
                    let n = chars[i + 1];
                    s.push(if n == 'n' { '\n' } else { n });
                    i += 2;
                } else if c == '"' {
                    i += 1;
                    closed = true;
                    break;
                } else {
                    s.push(c);
                    i += 1;
                }
            }
            if !closed {
                return Err("unterminated text \"…\"".into());
            }
            toks.push(Tok::Str(s));
        } else if ch.is_ascii_digit()
            || (ch == '.' && matches!(chars.get(i + 1), Some(c) if c.is_ascii_digit()))
        {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            let n: f64 = text.parse().map_err(|_| format!("bad number '{text}'"))?;
            toks.push(Tok::Num(n));
        } else if ch.is_ascii_alphabetic() || ch == '_' {
            // Identifier / dotted variable path (letters, digits, '_', '.').
            let start = i;
            while i < chars.len() {
                let c = chars[i];
                if c.is_ascii_alphanumeric() || c == '_' || c == '.' {
                    i += 1;
                } else {
                    break;
                }
            }
            let ident: String = chars[start..i].iter().collect();
            toks.push(Tok::Ident(ident));
        } else {
            // Operators and punctuation (two-char first).
            let op2 = match (ch, chars.get(i + 1)) {
                ('=', Some('=')) => Some("=="),
                ('!', Some('=')) => Some("!="),
                ('<', Some('=')) => Some("<="),
                ('>', Some('=')) => Some(">="),
                _ => None,
            };
            if let Some(op) = op2 {
                toks.push(Tok::Op(op));
                i += 2;
            } else {
                let t = match ch {
                    '+' => Tok::Op("+"),
                    '-' => Tok::Op("-"),
                    '*' => Tok::Op("*"),
                    '/' => Tok::Op("/"),
                    '%' => Tok::Op("%"),
                    '<' => Tok::Op("<"),
                    '>' => Tok::Op(">"),
                    '(' => Tok::LParen,
                    ')' => Tok::RParen,
                    ',' => Tok::Comma,
                    _ => return Err(format!("unexpected character '{ch}'")),
                };
                toks.push(t);
                i += 1;
            }
        }
    }
    Ok(toks)
}

// ---------------------------------------------------------------------------
// Parser (recursive descent by precedence)
// ---------------------------------------------------------------------------

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
    depth: usize,
    nodes: usize,
}

fn parse(src: &str) -> Result<Ast, String> {
    let toks = tokenize(src)?;
    if toks.is_empty() {
        return Err("empty formula".into());
    }
    let mut p = Parser {
        toks,
        pos: 0,
        depth: 0,
        nodes: 0,
    };
    let ast = p.parse_or()?;
    if p.pos != p.toks.len() {
        return Err(format!("unexpected '{}'", p.describe(p.pos)));
    }
    Ok(ast)
}

impl Parser {
    fn describe(&self, pos: usize) -> String {
        match self.toks.get(pos) {
            Some(Tok::Num(n)) => n.to_string(),
            Some(Tok::Str(s)) => format!("\"{s}\""),
            Some(Tok::Ident(s)) => s.clone(),
            Some(Tok::Op(o)) => (*o).to_string(),
            Some(Tok::LParen) => "(".into(),
            Some(Tok::RParen) => ")".into(),
            Some(Tok::Comma) => ",".into(),
            None => "end of formula".into(),
        }
    }

    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn enter(&mut self) -> Result<(), String> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err("formula nested too deeply".into());
        }
        Ok(())
    }
    fn leave(&mut self) {
        self.depth -= 1;
    }
    // Count an operator/call node. Bounds the total tree size, which caps a flat
    // chain the recursion-depth guard can't see (see MAX_NODES).
    fn node(&mut self) -> Result<(), String> {
        self.nodes += 1;
        if self.nodes > MAX_NODES {
            return Err("formula too complex".into());
        }
        Ok(())
    }

    // Consume an infix keyword ident like `and`/`or`.
    fn eat_kw(&mut self, kw: &str) -> bool {
        if matches!(self.peek(), Some(Tok::Ident(s)) if s == kw) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn eat_op(&mut self, op: &str) -> bool {
        if matches!(self.peek(), Some(Tok::Op(o)) if *o == op) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn parse_or(&mut self) -> Result<Ast, String> {
        self.enter()?;
        let mut left = self.parse_and()?;
        while self.eat_kw("or") {
            let right = self.parse_and()?;
            self.node()?;
            left = Ast::Bin(BinOp::Or, Box::new(left), Box::new(right));
        }
        self.leave();
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Ast, String> {
        let mut left = self.parse_not()?;
        while self.eat_kw("and") {
            let right = self.parse_not()?;
            self.node()?;
            left = Ast::Bin(BinOp::And, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    // `not` binds looser than comparison (like SQL/Python): `not a == b` is
    // `not (a == b)`, which is what a non-programmer expects — not `(not a) == b`.
    fn parse_not(&mut self) -> Result<Ast, String> {
        if self.eat_kw("not") {
            self.enter()?;
            let inner = self.parse_not()?;
            self.leave();
            self.node()?;
            Ok(Ast::Unary(UnOp::Not, Box::new(inner)))
        } else {
            self.parse_cmp()
        }
    }

    fn parse_cmp(&mut self) -> Result<Ast, String> {
        let left = self.parse_add()?;
        let op = match self.peek() {
            Some(Tok::Op(o @ ("==" | "!=" | "<" | "<=" | ">" | ">="))) => *o,
            _ => return Ok(left),
        };
        self.pos += 1;
        let right = self.parse_add()?;
        self.node()?;
        let b = match op {
            "==" => BinOp::Eq,
            "!=" => BinOp::Ne,
            "<" => BinOp::Lt,
            "<=" => BinOp::Le,
            ">" => BinOp::Gt,
            _ => BinOp::Ge,
        };
        Ok(Ast::Bin(b, Box::new(left), Box::new(right)))
    }

    fn parse_add(&mut self) -> Result<Ast, String> {
        let mut left = self.parse_mul()?;
        loop {
            let op = if self.eat_op("+") {
                BinOp::Add
            } else if self.eat_op("-") {
                BinOp::Sub
            } else {
                break;
            };
            let right = self.parse_mul()?;
            self.node()?;
            left = Ast::Bin(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_mul(&mut self) -> Result<Ast, String> {
        let mut left = self.parse_unary()?;
        loop {
            let op = if self.eat_op("*") {
                BinOp::Mul
            } else if self.eat_op("/") {
                BinOp::Div
            } else if self.eat_op("%") {
                BinOp::Mod
            } else {
                break;
            };
            let right = self.parse_unary()?;
            self.node()?;
            left = Ast::Bin(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Ast, String> {
        self.enter()?;
        let node = if self.eat_op("-") {
            self.node()?;
            Ast::Unary(UnOp::Neg, Box::new(self.parse_unary()?))
        } else {
            self.parse_primary()?
        };
        self.leave();
        Ok(node)
    }

    fn parse_primary(&mut self) -> Result<Ast, String> {
        match self.peek().cloned() {
            Some(Tok::Num(n)) => {
                self.pos += 1;
                Ok(Ast::Num(n))
            }
            Some(Tok::Str(s)) => {
                self.pos += 1;
                Ok(Ast::Str(s))
            }
            Some(Tok::LParen) => {
                self.pos += 1;
                let e = self.parse_or()?;
                if !matches!(self.peek(), Some(Tok::RParen)) {
                    return Err("expected ')'".into());
                }
                self.pos += 1;
                Ok(e)
            }
            Some(Tok::Ident(name)) => {
                self.pos += 1;
                match name.as_str() {
                    "true" => return Ok(Ast::Bool(true)),
                    "false" => return Ok(Ast::Bool(false)),
                    // A bare `and`/`or`/`not` here is a syntax error.
                    "and" | "or" | "not" => return Err(format!("unexpected '{name}'")),
                    _ => {}
                }
                if matches!(self.peek(), Some(Tok::LParen)) {
                    self.pos += 1;
                    let func = Func::from_name(&name)
                        .ok_or_else(|| format!("unknown function '{name}'"))?;
                    let mut args = Vec::new();
                    if !matches!(self.peek(), Some(Tok::RParen)) {
                        loop {
                            args.push(self.parse_or()?);
                            if self.eat_op_comma() {
                                continue;
                            }
                            break;
                        }
                    }
                    if !matches!(self.peek(), Some(Tok::RParen)) {
                        return Err(format!("expected ')' to close {name}(…)"));
                    }
                    self.pos += 1;
                    self.node()?;
                    Ok(Ast::Call(func, args))
                } else {
                    Ok(Ast::Var(name))
                }
            }
            other => Err(match other {
                Some(_) => format!("unexpected '{}'", self.describe(self.pos)),
                None => "unexpected end of formula".into(),
            }),
        }
    }

    fn eat_op_comma(&mut self) -> bool {
        if matches!(self.peek(), Some(Tok::Comma)) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Evaluator
// ---------------------------------------------------------------------------

fn eval(ast: &Ast, root: &Value, ctx: Scope, depth: usize, budget: &Cell<u64>) -> Value {
    if depth > MAX_NODES {
        return Value::Null;
    }
    let d = depth + 1;
    match ast {
        Ast::Num(n) => number_value(*n),
        Ast::Str(s) => Value::String(s.clone()),
        Ast::Bool(b) => Value::Bool(*b),
        // A missing path is null — NOT a fake. Inside a formula, "missing" must
        // stay empty so coalesce(), comparisons and aggregate filters behave
        // correctly. (A lone Variable element still fakes for a lively preview;
        // that happens in render.rs, not here.)
        Ast::Var(path) => resolve_scoped(ctx, root, path)
            .cloned()
            .unwrap_or(Value::Null),
        Ast::Unary(UnOp::Neg, a) => match to_number(&eval(a, root, ctx, d, budget)) {
            Some(n) => number_value(-n),
            None => Value::Null,
        },
        Ast::Unary(UnOp::Not, a) => Value::Bool(!as_bool(&eval(a, root, ctx, d, budget))),
        Ast::Bin(op, a, b) => eval_bin(*op, a, b, root, ctx, d, budget),
        Ast::Call(f, args) => eval_call(*f, args, root, ctx, d, budget),
    }
}

fn eval_bin(
    op: BinOp,
    a: &Ast,
    b: &Ast,
    root: &Value,
    ctx: Scope,
    d: usize,
    budget: &Cell<u64>,
) -> Value {
    // Logical operators short-circuit and work on truthiness.
    match op {
        BinOp::And => {
            return Value::Bool(
                as_bool(&eval(a, root, ctx, d, budget)) && as_bool(&eval(b, root, ctx, d, budget)),
            )
        }
        BinOp::Or => {
            return Value::Bool(
                as_bool(&eval(a, root, ctx, d, budget)) || as_bool(&eval(b, root, ctx, d, budget)),
            )
        }
        _ => {}
    }
    let (lv, rv) = (eval(a, root, ctx, d, budget), eval(b, root, ctx, d, budget));
    match op {
        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
            match (to_number(&lv), to_number(&rv)) {
                (Some(x), Some(y)) => match op {
                    BinOp::Add => number_value(x + y),
                    BinOp::Sub => number_value(x - y),
                    BinOp::Mul => number_value(x * y),
                    BinOp::Div if y != 0.0 => number_value(x / y),
                    BinOp::Mod if y != 0.0 => number_value(x % y),
                    _ => Value::Null, // divide / modulo by zero
                },
                _ => Value::Null, // non-numeric operand
            }
        }
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
            Value::Bool(compare(op, &lv, &rv))
        }
        BinOp::And | BinOp::Or => unreachable!(),
    }
}

/// Compare two values: numerically when both parse as numbers, else as text.
/// Shares `compare_strs` with `eval_condition` so a condition `x > y` and a
/// formula `x > y` can never disagree.
fn compare(op: BinOp, l: &Value, r: &Value) -> bool {
    use std::cmp::Ordering;
    let ord = compare_strs(&value_to_string(l), &value_to_string(r));
    match (op, ord) {
        (BinOp::Eq, Some(o)) => o == Ordering::Equal,
        (BinOp::Ne, Some(o)) => o != Ordering::Equal,
        (BinOp::Ne, None) => true, // NaN != anything
        (BinOp::Lt, Some(o)) => o == Ordering::Less,
        (BinOp::Le, Some(o)) => o != Ordering::Greater,
        (BinOp::Gt, Some(o)) => o == Ordering::Greater,
        (BinOp::Ge, Some(o)) => o != Ordering::Less,
        _ => false,
    }
}

/// Truthiness for `and`/`or`/`not` and aggregate filters.
fn as_bool(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Null => false,
        Value::Number(n) => n.as_f64().map(|x| x != 0.0).unwrap_or(false),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(_) => true,
    }
}

fn eval_call(
    f: Func,
    args: &[Ast],
    root: &Value,
    ctx: Scope,
    d: usize,
    budget: &Cell<u64>,
) -> Value {
    match f {
        Func::Concat => {
            let mut s = String::new();
            for a in args {
                s.push_str(&value_to_string(&eval(a, root, ctx, d, budget)));
            }
            Value::String(s)
        }
        Func::Round => {
            if args.len() != 2 {
                return Value::Null;
            }
            match (
                to_number(&eval(&args[0], root, ctx, d, budget)),
                to_number(&eval(&args[1], root, ctx, d, budget)),
            ) {
                (Some(x), Some(dec)) => {
                    let dec = dec.clamp(0.0, 10.0) as i32;
                    let f = 10f64.powi(dec);
                    number_value((x * f).round() / f)
                }
                _ => Value::Null,
            }
        }
        Func::Abs => match args
            .first()
            .map(|a| to_number(&eval(a, root, ctx, d, budget)))
        {
            Some(Some(x)) => number_value(x.abs()),
            _ => Value::Null,
        },
        Func::Min | Func::Max => {
            let nums: Vec<f64> = args
                .iter()
                .filter_map(|a| to_number(&eval(a, root, ctx, d, budget)))
                .collect();
            match nums.into_iter().reduce(|acc, x| {
                if matches!(f, Func::Min) {
                    acc.min(x)
                } else {
                    acc.max(x)
                }
            }) {
                Some(x) => number_value(x),
                None => Value::Null,
            }
        }
        Func::Coalesce => {
            for a in args {
                let v = eval(a, root, ctx, d, budget);
                if !is_empty_value(&v) {
                    return v;
                }
            }
            Value::Null
        }
        Func::Count | Func::CountIf | Func::Sum | Func::SumIf | Func::Avg | Func::AvgIf => {
            eval_aggregate(f, args, root, ctx, d, budget)
        }
    }
}

/// Aggregate over a list variable. The first argument must be a list variable;
/// remaining arguments (a filter condition and/or a value expression) are
/// evaluated once per row with the row bound as the current loop item, so bare
/// field names inside them resolve against that row.
fn eval_aggregate(
    f: Func,
    args: &[Ast],
    root: &Value,
    ctx: Scope,
    d: usize,
    budget: &Cell<u64>,
) -> Value {
    // Expected arities: count(list) / countif(list,cond) / sum(list,value) /
    // sumif(list,cond,value) / avg(list,value) / avgif(list,cond,value).
    let (want, has_cond, has_value) = match f {
        Func::Count => (1, false, false),
        Func::CountIf => (2, true, false),
        Func::Sum | Func::Avg => (2, false, true),
        Func::SumIf | Func::AvgIf => (3, true, true),
        _ => return Value::Null,
    };
    if args.len() != want {
        return Value::Null;
    }
    let path = match &args[0] {
        Ast::Var(p) => p.as_str(),
        _ => return Value::Null,
    };
    // Resolve the list by reference — no per-row clone. Absent → empty list.
    let items: &[Value] = match resolve_scoped(ctx, root, path) {
        Some(Value::Array(a)) => a.as_slice(),
        Some(_) => return Value::Null, // present but not a list
        None => &[],
    };
    let cond = if has_cond { Some(&args[1]) } else { None };
    let value = if has_value { args.last() } else { None };

    let mut count: u64 = 0;
    let mut sum = 0.0f64;
    let mut numeric = 0u64;
    for (i, item) in items.iter().take(MAX_ROWS).enumerate() {
        // Charge the shared budget so nested aggregates can't multiply into a
        // stall; once spent, stop scanning (a bounded, deterministic result).
        let remaining = budget.get();
        if remaining == 0 {
            break;
        }
        budget.set(remaining - 1);
        // The aggregate's row becomes the loop binding; the surrounding band's
        // `row.*` values stay visible (e.g. a filter comparing against
        // `row.importe` of the outer iteration).
        let row_ctx = Scope {
            loop_ctx: Some((path, i, item)),
            row: ctx.row,
        };
        if let Some(c) = cond {
            if !as_bool(&eval(c, root, row_ctx, d, budget)) {
                continue;
            }
        }
        count += 1;
        if let Some(v) = value {
            if let Some(n) = to_number(&eval(v, root, row_ctx, d, budget)) {
                sum += n;
                numeric += 1;
            }
        }
    }
    match f {
        Func::Count | Func::CountIf => number_value(count as f64),
        Func::Sum | Func::SumIf => number_value(sum),
        Func::Avg | Func::AvgIf => {
            if numeric == 0 {
                Value::Null
            } else {
                number_value(sum / numeric as f64)
            }
        }
        _ => Value::Null,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ev(formula: &str, data: &Value) -> Value {
        eval_formula(formula, data, Scope::default()).unwrap()
    }

    #[test]
    fn literals_arithmetic_and_precedence() {
        let d = json!({ "a": 10, "b": 4 });
        assert_eq!(ev("a + b * 2", &d), json!(18)); // * before +
        assert_eq!(ev("(a + b) * 2", &d), json!(28));
        assert_eq!(ev("a - b - 1", &d), json!(5)); // left-assoc
        assert_eq!(ev("a / b", &d), json!(2.5));
        assert_eq!(ev("a % b", &d), json!(2));
        assert_eq!(ev("-a + 3", &d), json!(-7));
    }

    #[test]
    fn concat_and_strings() {
        let d = json!({ "lat": "19.4", "lng": "-99.1" });
        assert_eq!(
            ev(r#"concat("q=", lat, ",", lng)"#, &d),
            json!("q=19.4,-99.1")
        );
    }

    #[test]
    fn float_noise_is_cleaned() {
        let d = json!({ "a": 46.24, "b": 4.62 });
        assert_eq!(ev("a - b", &d), json!(41.62));
    }

    #[test]
    fn comparisons_and_logic() {
        let d = json!({ "x": 5, "p": "CASH" });
        assert_eq!(ev("x > 3", &d), json!(true));
        assert_eq!(ev(r#"p == "CASH""#, &d), json!(true));
        assert_eq!(ev(r#"p == "CASH" and x > 3"#, &d), json!(true));
        assert_eq!(ev(r#"p == "CARD" or x > 10"#, &d), json!(false));
        assert_eq!(ev(r#"not (p == "CARD")"#, &d), json!(true));
    }

    #[test]
    fn scalar_functions() {
        let d = json!({});
        assert_eq!(ev("round(1.23456, 2)", &d), json!(1.23));
        assert_eq!(ev("min(3, 7, 2)", &d), json!(2));
        assert_eq!(ev("max(3, 7, 2)", &d), json!(7));
        assert_eq!(ev("abs(0 - 5)", &d), json!(5));
        assert_eq!(
            ev(r#"coalesce("", missing.thing, "fallback")"#, &d),
            json!("fallback")
        );
    }

    #[test]
    fn aggregates_over_a_list_with_filters() {
        let d = json!({ "movs": [
            { "payment": "CASH", "qty": 3 },
            { "payment": "CARD", "qty": 5 },
            { "payment": "CASH", "qty": 2 },
        ]});
        assert_eq!(ev("count(movs)", &d), json!(3));
        assert_eq!(ev(r#"countif(movs, payment == "CASH")"#, &d), json!(2));
        assert_eq!(ev("sum(movs, qty)", &d), json!(10));
        assert_eq!(ev(r#"sumif(movs, payment == "CASH", qty)"#, &d), json!(5));
        assert_eq!(ev(r#"sumif(movs, payment == "CARD", qty)"#, &d), json!(5));
        // (3 + 5 + 2) / 3 = 3.3333333333 (float noise rounded off).
        assert_eq!(ev("avg(movs, qty)", &d), json!(3.3333333333));
        // count of an absent list is 0, not an error.
        assert_eq!(ev("count(nope)", &d), json!(0));
    }

    #[test]
    fn parse_errors_are_reported() {
        assert!(eval_formula("1 +", &Value::Null, Scope::default()).is_err());
        assert!(eval_formula("foo(1)", &Value::Null, Scope::default()).is_err()); // unknown function
        assert!(eval_formula("(1 + 2", &Value::Null, Scope::default()).is_err()); // unbalanced
        assert!(eval_formula(r#""oops"#, &Value::Null, Scope::default()).is_err()); // unterminated string
    }

    #[test]
    fn deeply_nested_is_rejected_not_stack_overflow() {
        let bomb = "(".repeat(200);
        assert!(eval_formula(&bomb, &Value::Null, Scope::default()).is_err());
    }

    #[test]
    fn non_ascii_input_does_not_panic() {
        // Multibyte chars in a literal and as stray tokens must not panic the
        // tokenizer (byte-slicing would). Accented literal text round-trips
        // intact (no mojibake); stray non-ASCII yields a clean error.
        assert_eq!(
            ev(r#"concat("café €", " ok")"#, &Value::Null),
            json!("café € ok")
        );
        assert!(eval_formula("1 + €", &Value::Null, Scope::default()).is_err()); // stray 3-byte char
        assert!(eval_formula("😀", &Value::Null, Scope::default()).is_err()); // stray 4-byte char
        assert!(eval_formula("1 <€", &Value::Null, Scope::default()).is_err()); // multibyte after an operator
    }

    #[test]
    fn long_flat_chain_errors_instead_of_silently_nulling() {
        // A chain longer than MAX_NODES must fail to PARSE with a clear error,
        // not parse-then-evaluate-to-blank.
        let terms = (0..2000).map(|_| "1").collect::<Vec<_>>().join(" + ");
        assert!(eval_formula(&terms, &Value::Null, Scope::default()).is_err());
        // A reasonable-length chain still evaluates correctly.
        let ok = (0..40).map(|_| "1").collect::<Vec<_>>().join(" + ");
        assert_eq!(ev(&ok, &Value::Null), json!(40));
    }

    #[test]
    fn not_binds_looser_than_comparison() {
        // `not a == b` must be `not (a == b)`, matching SQL/Python intent.
        let d = json!({ "paid": 0 });
        assert_eq!(ev("not paid == 1", &d), json!(true)); // not (0 == 1) = true
        assert_eq!(ev("not paid == 0", &d), json!(false));
    }

    #[test]
    fn nested_aggregate_total_row_budget_is_bounded() {
        // Two 1000-row lists nested would be 10^6 row-evals; the shared budget
        // (2M) keeps it finite and it must not hang or panic — just return a
        // (bounded) number.
        let big: Vec<_> = (0..1000).map(|i| json!({ "v": i })).collect();
        let d = json!({ "a": big.clone(), "b": big });
        let out = ev("sumif(a, countif(b, v > 0) > 0, v)", &d);
        assert!(
            out.is_number(),
            "nested aggregate must return a number, got {out:?}"
        );
    }
}
