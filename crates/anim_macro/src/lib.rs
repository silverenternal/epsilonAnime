mod expr;
mod kernel;

use proc_macro::TokenStream;
use quote::quote;
use std::collections::{BTreeMap, BTreeSet};
use syn::parse::{Parse, ParseStream};
use syn::{Ident, LitStr, Result, Token};

struct KernelInput {
    name: Ident,
    params: BTreeMap<String, f64>,
    dynamic: BTreeSet<String>,
    expr: LitStr,
}

impl Parse for KernelInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut name = None;
        let mut params = BTreeMap::new();
        let mut dynamic = BTreeSet::new();
        let mut expr = None;
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            match key.to_string().as_str() {
                "name" => name = Some(input.parse()?),
                "expr" => expr = Some(input.parse()?),
                "params" => {
                    let content;
                    syn::braced!(content in input);
                    while !content.is_empty() {
                        let p: Ident = content.parse()?;
                        content.parse::<Token![=]>()?;
                        let value = kernel::number_expr(&content.parse()?)?;
                        params.insert(p.to_string(), value);
                        if content.is_empty() {
                            break;
                        }
                        content.parse::<Token![,]>()?;
                    }
                }
                "dynamic" => {
                    let content;
                    syn::braced!(content in input);
                    while !content.is_empty() {
                        dynamic.insert(content.parse::<Ident>()?.to_string());
                        if content.is_empty() {
                            break;
                        }
                        content.parse::<Token![,]>()?;
                    }
                }
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown field `{other}`"),
                    ));
                }
            }
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }
        Ok(Self {
            name: name.ok_or_else(|| syn::Error::new(input.span(), "missing name"))?,
            params,
            dynamic,
            expr: expr.ok_or_else(|| syn::Error::new(input.span(), "missing expr"))?,
        })
    }
}

#[proc_macro]
pub fn math_kernel(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as KernelInput);
    let ast = match expr::Parser::parse(&input.expr.value()) {
        Ok(ast) => kernel::fold(&ast, &input.params, &input.dynamic),
        Err(err) => {
            return syn::Error::new(input.expr.span(), err)
                .to_compile_error()
                .into();
        }
    };
    let (min, max) = kernel::sample_range(&ast, &input.params, &input.dynamic);
    let rust_expr = match kernel::emit(&ast, &input.params, &input.dynamic) {
        Ok(code) => code,
        Err(err) => {
            return syn::Error::new(input.expr.span(), err)
                .to_compile_error()
                .into();
        }
    };
    let fn_name = &input.name;
    let rust_tokens = match syn::parse_str::<syn::Expr>(&rust_expr) {
        Ok(expr) => expr,
        Err(err) => {
            return syn::Error::new(input.expr.span(), err.to_string())
                .to_compile_error()
                .into();
        }
    };
    let min_ident = syn::Ident::new(
        &format!("{}_MIN", fn_name.to_string().to_uppercase()),
        fn_name.span(),
    );
    let max_ident = syn::Ident::new(
        &format!("{}_MAX", fn_name.to_string().to_uppercase()),
        fn_name.span(),
    );
    let min_lit = proc_macro2::Literal::f64_suffixed(min);
    let max_lit = proc_macro2::Literal::f64_suffixed(max);
    let span = fn_name.span();
    let dynamic_args: Vec<Ident> = input
        .dynamic
        .iter()
        .map(|name| Ident::new(name, span))
        .collect();
    let generated = quote::quote_spanned! {span=>
        pub const #min_ident: f32 = #min_lit as f32;
        pub const #max_ident: f32 = #max_lit as f32;
        #[allow(clippy::excessive_precision, clippy::approx_constant)]
        pub fn #fn_name(x: f32, y: f32, t: f32, #(#dynamic_args: f32),*) -> f32 {
            let raw: f32 = #rust_tokens;
            let range = (#max_ident - #min_ident).max(1.0e-6);
            ((raw - #min_ident) / range).clamp(0.0, 1.0)
        }
    };
    quote!(#generated).into()
}
