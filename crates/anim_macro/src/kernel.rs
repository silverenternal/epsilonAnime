use crate::expr::Expr;
use std::collections::{BTreeMap, BTreeSet};

pub fn number_expr(expr: &syn::Expr) -> syn::Result<f64> {
    match expr {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Float(v),
            ..
        }) => v.base10_parse(),
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(v),
            ..
        }) => v.base10_parse::<i64>().map(|v| v as f64),
        syn::Expr::Unary(syn::ExprUnary {
            op: syn::UnOp::Neg(_),
            expr,
            ..
        }) => Ok(-number_expr(expr)?),
        _ => Err(syn::Error::new_spanned(
            expr,
            "parameter must be a numeric literal",
        )),
    }
}

pub fn eval(expr: &Expr, x: f64, y: f64, t: f64, p: &BTreeMap<String, f64>) -> f64 {
    match expr {
        Expr::Num(v) => *v,
        Expr::Var(name) => match name.as_str() {
            "x" => x,
            "y" => y,
            "t" => t,
            "PI" => std::f64::consts::PI,
            "TAU" => std::f64::consts::TAU,
            other => *p.get(other).unwrap_or(&0.0),
        },
        Expr::Unary(op, inner) => {
            let v = eval(inner, x, y, t, p);
            if *op == '-' { -v } else { v }
        }
        Expr::Binary(op, lhs, rhs) => {
            let a = eval(lhs, x, y, t, p);
            let b = eval(rhs, x, y, t, p);
            match op {
                '+' => a + b,
                '-' => a - b,
                '*' => a * b,
                '/' => a / b,
                '%' => a % b,
                '^' => a.powf(b),
                _ => f64::NAN,
            }
        }
        Expr::Call(name, args) => {
            let a = |i: usize| args.get(i).map(|e| eval(e, x, y, t, p)).unwrap_or(0.0);
            match name.as_str() {
                "sin" => a(0).sin(),
                "cos" => a(0).cos(),
                "tan" => a(0).tan(),
                "exp" => a(0).exp(),
                "ln" => a(0).ln(),
                "sqrt" => a(0).sqrt(),
                "abs" => a(0).abs(),
                "floor" => a(0).floor(),
                "fract" => a(0).fract(),
                "atan2" => a(0).atan2(a(1)),
                "min" => a(0).min(a(1)),
                "max" => a(0).max(a(1)),
                "clamp" => a(0).clamp(a(1), a(2)),
                "pow" => a(0).powf(a(1)),
                "bessel_j" => puruspe::Jn(a(0).max(0.0) as u32, a(1).max(0.0)),
                "bessel_y" => puruspe::Yn(a(0).max(0.0) as u32, a(1).max(1.0e-6)),
                "bessel_i" => puruspe::In(a(0).max(0.0) as u32, a(1).max(0.0)),
                "bessel_k" => puruspe::Kn(a(0).max(0.0) as u32, a(1).max(1.0e-6)),
                "erf" => puruspe::erf(a(0)),
                "erfc" => puruspe::erfc(a(0)),
                "gamma" => puruspe::gamma(a(0)),
                "ln_gamma" => puruspe::ln_gamma(a(0)),
                _ => f64::NAN,
            }
        }
    }
}

pub fn fold(expr: &Expr, params: &BTreeMap<String, f64>, dynamic: &BTreeSet<String>) -> Expr {
    match expr {
        Expr::Num(v) => Expr::Num(*v),
        Expr::Var(name) if name != "x" && name != "y" && name != "t" && !dynamic.contains(name) => {
            Expr::Num(eval(expr, 0.0, 0.0, 0.0, params))
        }
        Expr::Var(name) => Expr::Var(name.clone()),
        Expr::Unary(op, inner) => {
            let inner = fold(inner, params, dynamic);
            match inner {
                Expr::Num(v) => Expr::Num(if *op == '-' { -v } else { v }),
                other => Expr::Unary(*op, Box::new(other)),
            }
        }
        Expr::Binary(op, lhs, rhs) => {
            let lhs = fold(lhs, params, dynamic);
            let rhs = fold(rhs, params, dynamic);
            if let (Expr::Num(_), Expr::Num(_)) = (&lhs, &rhs) {
                Expr::Num(eval(
                    &Expr::Binary(*op, Box::new(lhs.clone()), Box::new(rhs.clone())),
                    0.0,
                    0.0,
                    0.0,
                    params,
                ))
            } else {
                Expr::Binary(*op, Box::new(lhs), Box::new(rhs))
            }
        }
        Expr::Call(name, args) => {
            let args: Vec<Expr> = args.iter().map(|a| fold(a, params, dynamic)).collect();
            if args.iter().all(|a| matches!(a, Expr::Num(_))) {
                Expr::Num(eval(
                    &Expr::Call(name.clone(), args.clone()),
                    0.0,
                    0.0,
                    0.0,
                    params,
                ))
            } else {
                Expr::Call(name.clone(), args)
            }
        }
    }
}

pub fn sample_range(
    ast: &Expr,
    params: &BTreeMap<String, f64>,
    dynamic: &BTreeSet<String>,
) -> (f64, f64) {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for step in 0..=2 {
        let mut values = params.clone();
        for name in dynamic {
            values.insert(name.clone(), step as f64 / 2.0);
        }
        for yi in 0..=16 {
            for xi in 0..=16 {
                for ti in 0..=6 {
                    let v = eval(
                        ast,
                        xi as f64 / 16.0,
                        yi as f64 / 16.0,
                        ti as f64 / 6.0,
                        &values,
                    );
                    if v.is_finite() {
                        min = min.min(v);
                        max = max.max(v);
                    }
                }
            }
        }
    }
    if !min.is_finite() || !max.is_finite() || (max - min).abs() < 1.0e-9 {
        (-1.0, 1.0)
    } else {
        (min, max)
    }
}

fn fmt(v: f64) -> String {
    format!("{v:.9}f32")
}

pub fn emit(
    expr: &Expr,
    p: &BTreeMap<String, f64>,
    dynamic: &BTreeSet<String>,
) -> Result<String, String> {
    Ok(match expr {
        Expr::Num(v) => fmt(*v),
        Expr::Var(name) => match name.as_str() {
            "x" | "y" | "t" => name.clone(),
            "PI" => "std::f32::consts::PI".into(),
            "TAU" => "std::f32::consts::TAU".into(),
            other => p
                .get(other)
                .map(|v| fmt(*v))
                .or_else(|| dynamic.contains(other).then(|| other.to_string()))
                .ok_or_else(|| format!("unknown identifier `{other}`"))?,
        },
        Expr::Unary(op, inner) => format!("({}{})", op, emit(inner, p, dynamic)?),
        Expr::Binary('^', lhs, rhs) => {
            format!(
                "({}).powf({})",
                emit(lhs, p, dynamic)?,
                emit(rhs, p, dynamic)?
            )
        }
        Expr::Binary(op, lhs, rhs) => {
            format!(
                "({} {} {})",
                emit(lhs, p, dynamic)?,
                op,
                emit(rhs, p, dynamic)?
            )
        }
        Expr::Call(name, args) => {
            let code: Vec<String> = args
                .iter()
                .map(|a| emit(a, p, dynamic))
                .collect::<Result<_, _>>()?;
            let a = |i: usize| code.get(i).cloned().unwrap_or_else(|| "0.0f32".into());
            match name.as_str() {
                "sin" => format!("({}).sin()", a(0)),
                "cos" => format!("({}).cos()", a(0)),
                "tan" => format!("({}).tan()", a(0)),
                "exp" => format!("({}).exp()", a(0)),
                "ln" => format!("({}).ln()", a(0)),
                "sqrt" => format!("({}).sqrt()", a(0)),
                "abs" => format!("({}).abs()", a(0)),
                "floor" => format!("({}).floor()", a(0)),
                "fract" => format!("({}).fract()", a(0)),
                "atan2" => format!("({}).atan2({})", a(0), a(1)),
                "min" => format!("f32::min({}, {})", a(0), a(1)),
                "max" => format!("f32::max({}, {})", a(0), a(1)),
                "clamp" => format!("({}).clamp({}, {})", a(0), a(1), a(2)),
                "pow" => format!("({}).powf({})", a(0), a(1)),
                "bessel_j" => format!("crate::mathlib::bessel_j({}, {})", a(0), a(1)),
                "bessel_y" => format!("crate::mathlib::bessel_y({}, {})", a(0), a(1)),
                "bessel_i" => format!("crate::mathlib::bessel_i({}, {})", a(0), a(1)),
                "bessel_k" => format!("crate::mathlib::bessel_k({}, {})", a(0), a(1)),
                "erf" => format!("crate::mathlib::erf({})", a(0)),
                "erfc" => format!("crate::mathlib::erfc({})", a(0)),
                "gamma" => format!("crate::mathlib::gamma({})", a(0)),
                "ln_gamma" => format!("crate::mathlib::ln_gamma({})", a(0)),
                _ => return Err(format!("unknown function `{name}`")),
            }
        }
    })
}
