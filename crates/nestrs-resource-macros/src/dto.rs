//! Emit the GraphQL output object for the entity — the `name` given to
//! `#[expose]` — plus its `From<&Model>`. A `skip` field is absent; a `Uuid`
//! renders as `String`. The type derives `SimpleObject` (GraphQL) and
//! `JsonSchema` (OpenAPI), so one declaration on the entity feeds both surfaces.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use crate::attr::{is_uuid, ResourceModel};

pub fn emit(model: &ResourceModel) -> TokenStream2 {
    let output = &model.output_ident;
    let source = &model.source_ident;
    let mut decls = Vec::new();
    let mut inits = Vec::new();

    for field in model.fields.iter().filter(|f| f.in_output()) {
        let name = &field.ident;
        if is_uuid(&field.ty) {
            decls.push(quote! { pub #name: ::std::string::String });
            inits.push(quote! { #name: ::std::string::ToString::to_string(&model.#name) });
        } else {
            let ty = &field.ty;
            decls.push(quote! { pub #name: #ty });
            inits.push(quote! { #name: ::core::clone::Clone::clone(&model.#name) });
        }
    }

    let complex = if model.complex {
        quote! { #[graphql(complex)] }
    } else {
        quote! {}
    };

    quote! {
        #[derive(
            ::core::fmt::Debug,
            ::core::clone::Clone,
            ::serde::Serialize,
            ::nestrs_graphql::async_graphql::SimpleObject,
            ::schemars::JsonSchema,
        )]
        #complex
        pub struct #output {
            #(#decls),*
        }

        impl ::core::convert::From<&#source> for #output {
            fn from(model: &#source) -> Self {
                Self { #(#inits),* }
            }
        }
    }
}
