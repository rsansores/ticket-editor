//! `#[derive(Printable)]` — the projection from an annotated struct to the
//! `variables` JSON a [`TicketDoc`](ticket_core) renders against.
//!
//! The derive is deliberately dumb: it walks the struct's named fields and, for
//! each one the host hasn't hidden, emits three parallel things —
//!   * a key in `to_value()` (the real data at render time),
//!   * a key in `sample_json()` (placeholder data for the editor preview),
//!   * an entry in `var_types()` (leaf → `VarType`, so the editor offers the
//!     right formatting).
//!
//! It knows nothing about databases, web frameworks, or the field types
//! themselves — every field is treated uniformly as `T: Printable`, so a leaf
//! (`String`, `Decimal`, …) and a nested `#[derive(Printable)]` struct compose
//! the same way. Assembling the struct from real rows is the host's job.
//!
//! ## Field policy — pure denylist
//!
//! Every field is a variable **unless** marked `#[printable(hidden)]`. A new
//! column added to a model becomes available in the editor automatically; there
//! is nothing to remember to update. Hide the handful of internal fields (ids,
//! foreign keys, sync flags, secrets) explicitly.
//!
//! ## Flattening
//!
//! `#[printable(flatten)]` lifts a nested struct's variables to the level of
//! the struct that holds it, as `serde(flatten)` does — so a read model that
//! wraps a row (`{ unit: Unit, address: Option<Address> }`) prints
//! `reception_unit.code` and `reception_unit.address.street`, not
//! `reception_unit.unit.code`. The flattened field must project to an object;
//! one that projects to anything else (an `Option` that is `None`) adds nothing.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

/// Derive [`ticket_printable::Printable`] for a struct with named fields.
///
/// Field attributes: `#[printable(hidden)]` excludes the field from every
/// projection; `#[printable(flatten)]` lifts its variables to this struct's
/// level.
#[proc_macro_derive(Printable, attributes(printable))]
pub fn derive_printable(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let (impl_g, ty_g, where_g) = input.generics.split_for_impl();

    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(named) => &named.named,
            _ => {
                return syn::Error::new_spanned(
                    name,
                    "Printable can only be derived for structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(name, "Printable can only be derived for structs")
                .to_compile_error()
                .into();
        }
    };

    let mut to_value = Vec::new();
    let mut sample = Vec::new();
    let mut var_types = Vec::new();

    for f in fields {
        if is_hidden(f) {
            continue;
        }
        let ident = f.ident.as_ref().expect("named field");
        let ty = &f.ty;
        let key = ident.to_string();

        if has_flag(f, "flatten") {
            to_value.push(quote! {
                if let ::ticket_printable::serde_json::Value::Object(__inner) =
                    ::ticket_printable::Printable::to_value(&self.#ident)
                {
                    __map.extend(__inner);
                }
            });
            sample.push(quote! {
                if let ::ticket_printable::serde_json::Value::Object(__inner) =
                    <#ty as ::ticket_printable::Printable>::sample_json()
                {
                    __map.extend(__inner);
                }
            });
            var_types.push(quote! {
                <#ty as ::ticket_printable::Printable>::var_types(__prefix, __out);
            });
            continue;
        }

        to_value.push(quote! {
            __map.insert(#key.to_string(), ::ticket_printable::Printable::to_value(&self.#ident));
        });
        sample.push(quote! {
            __map.insert(#key.to_string(), <#ty as ::ticket_printable::Printable>::sample_json());
        });
        var_types.push(quote! {
            <#ty as ::ticket_printable::Printable>::var_types(
                &::std::format!("{}{}.", __prefix, #key),
                __out,
            );
        });
    }

    quote! {
        impl #impl_g ::ticket_printable::Printable for #name #ty_g #where_g {
            fn to_value(&self) -> ::ticket_printable::serde_json::Value {
                let mut __map = ::ticket_printable::serde_json::Map::new();
                #(#to_value)*
                ::ticket_printable::serde_json::Value::Object(__map)
            }
            fn sample_json() -> ::ticket_printable::serde_json::Value {
                let mut __map = ::ticket_printable::serde_json::Map::new();
                #(#sample)*
                ::ticket_printable::serde_json::Value::Object(__map)
            }
            fn var_types(
                __prefix: &str,
                __out: &mut ::std::collections::BTreeMap<::std::string::String, ::ticket_printable::VarType>,
            ) {
                #(#var_types)*
            }
        }
    }
    .into()
}

/// True when the field carries `#[printable(hidden)]`.
fn is_hidden(field: &syn::Field) -> bool {
    has_flag(field, "hidden")
}

/// True when the field's `#[printable(...)]` holds `flag`.
fn has_flag(field: &syn::Field, flag: &str) -> bool {
    let mut found = false;
    for attr in &field.attrs {
        if !attr.path().is_ident("printable") {
            continue;
        }
        // Best-effort parse inside `#[printable(...)]`. Unknown tokens are
        // ignored so the vocabulary can grow without breaking.
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident(flag) {
                found = true;
            }
            Ok(())
        });
    }
    found
}
