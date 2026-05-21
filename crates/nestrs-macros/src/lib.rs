use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{
    bracketed, parse_macro_input, Fields, FnArg, GenericArgument, Ident, ImplItem, ItemImpl,
    ItemStruct, LitStr, Pat, Path, PathArguments, ReturnType, Token, Type,
};

// -----------------------------------------------------------------------------
// #[injectable]
// -----------------------------------------------------------------------------

/// Mark a struct as a provider that can be constructed from the IoC container.
///
/// - Fields tagged `#[inject]` are resolved via `container.get()`.
/// - Other fields fall back to `Default::default()`.
/// - If no field carries `#[inject]`, the macro defers to `<Self as Default>::default()`
///   so any custom `Default` impl on the struct is preserved.
///
/// Also emits `impl Discoverable for Self` so the struct is usable directly
/// in `#[module(providers = [...])]`. The registration simply builds the
/// value via `from_container` and stores it via `ContainerBuilder::provide`.
#[proc_macro_attribute]
pub fn injectable(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut item = parse_macro_input!(input as ItemStruct);

    let body = match build_injectable_body(&mut item) {
        Ok(body) => body,
        Err(err) => return err.to_compile_error().into(),
    };

    let name = item.ident.clone();
    let (impl_generics, ty_generics, where_clause) = item.generics.split_for_impl();

    quote! {
        #item

        impl #impl_generics #name #ty_generics #where_clause {
            pub fn from_container(container: &::nestrs_core::Container) -> Self {
                let _ = container;
                #body
            }
        }

        impl #impl_generics ::nestrs_core::Discoverable for #name #ty_generics #where_clause {
            fn register(
                builder: ::nestrs_core::ContainerBuilder,
            ) -> ::nestrs_core::ContainerBuilder {
                let __snapshot = builder.snapshot();
                let __value = Self::from_container(&__snapshot);
                builder.provide(__value)
            }
        }
    }
    .into()
}

/// If `ty` syntactically matches `Arc<dyn Trait + ...>`, return the inner
/// trait-object type so the macro can emit a `get_dyn::<dyn Trait + ...>()`
/// call. Only the last path segment is inspected (`std::sync::Arc<dyn T>`
/// works as well as `Arc<dyn T>`).
fn arc_dyn_inner(ty: &Type) -> Option<&Type> {
    let Type::Path(tp) = ty else { return None };
    let seg = tp.path.segments.last()?;
    if seg.ident != "Arc" {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &seg.arguments else {
        return None;
    };
    if args.args.len() != 1 {
        return None;
    }
    let GenericArgument::Type(inner) = &args.args[0] else {
        return None;
    };
    matches!(inner, Type::TraitObject(_)).then_some(inner)
}

fn build_injectable_body(item: &mut ItemStruct) -> syn::Result<TokenStream2> {
    match &mut item.fields {
        Fields::Unit => Ok(quote! { Self }),
        Fields::Named(fields) => {
            let mut has_inject = false;
            let mut field_inits = Vec::new();

            for field in fields.named.iter_mut() {
                let field_name = field.ident.clone().expect("named field has an ident");
                let inject_idx = field.attrs.iter().position(|a| a.path().is_ident("inject"));
                if let Some(idx) = inject_idx {
                    field.attrs.remove(idx);
                    has_inject = true;
                    let msg = format!(
                        "{}.{}: no provider registered for this dependency",
                        item.ident, field_name
                    );
                    if let Some(trait_ty) = arc_dyn_inner(&field.ty) {
                        field_inits.push(quote! {
                            #field_name: container.get_dyn::<#trait_ty>().expect(#msg)
                        });
                    } else {
                        field_inits.push(quote! {
                            #field_name: container.get().expect(#msg)
                        });
                    }
                } else {
                    field_inits.push(quote! {
                        #field_name: ::core::default::Default::default()
                    });
                }
            }

            if has_inject {
                Ok(quote! { Self { #(#field_inits),* } })
            } else {
                Ok(quote! { <Self as ::core::default::Default>::default() })
            }
        }
        Fields::Unnamed(_) => Err(syn::Error::new_spanned(
            &item.fields,
            "#[injectable] does not support tuple structs",
        )),
    }
}

// -----------------------------------------------------------------------------
// #[module]
// -----------------------------------------------------------------------------

/// `#[module(imports = [...], providers = [...])]`.
///
/// Both keys are optional. `imports` lists other modules to compose in,
/// each contributing their own providers and metadata via `Module::register`.
/// `providers` lists everything this module declares — services,
/// controllers, interceptors, future cron jobs / event handlers / MCP tools.
///
/// Each provider entry is one of:
///
/// - `Foo` — a concrete type that implements `Discoverable` (every
///   `#[injectable]`, `#[controller]`+`#[routes]`, and `#[interceptor]`
///   struct does). The macro expands to a single
///   `<Foo as Discoverable>::register(builder)` call.
/// - `Foo as dyn Trait` — a trait-object binding. The macro builds `Foo`
///   from a snapshot and stores it under the trait's `TypeId` via
///   `provide_dyn`, so dependents can inject `Arc<dyn Trait>`.
///
/// Order matters: entries register in the order they appear, against a
/// snapshot of the container including all earlier entries (and all
/// imports). Put dependencies before their consumers.
#[proc_macro_attribute]
pub fn module(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as ModuleArgs);
    let item = parse_macro_input!(input as ItemStruct);
    let name = item.ident.clone();

    let import_calls = args.imports.iter().map(|p| {
        quote! { builder = <#p as ::nestrs_core::Module>::register(builder); }
    });

    let provider_calls = args.providers.iter().map(|binding| match binding {
        ProviderBinding::Concrete(p) => quote! {
            builder = <#p as ::nestrs_core::Discoverable>::register(builder);
        },
        ProviderBinding::Dyn { provider, trait_ty } => quote! {
            {
                let __snapshot = builder.snapshot();
                let __provider = #provider::from_container(&__snapshot);
                let __dyn: ::std::sync::Arc<#trait_ty> =
                    ::std::sync::Arc::new(__provider);
                builder = builder.provide_dyn::<#trait_ty>(__dyn);
            }
        },
    });

    quote! {
        #item

        impl ::nestrs_core::Module for #name {
            fn register(
                mut builder: ::nestrs_core::ContainerBuilder,
            ) -> ::nestrs_core::ContainerBuilder {
                #(#import_calls)*
                #(#provider_calls)*
                builder
            }
        }
    }
    .into()
}

#[derive(Default)]
struct ModuleArgs {
    imports: Vec<Path>,
    providers: Vec<ProviderBinding>,
}

/// Either a concrete provider (`MyService`) or a trait-object binding
/// (`MyService as dyn MyTrait`). The latter registers the value under the
/// trait's `TypeId` so dependents can inject `Arc<dyn MyTrait>`.
enum ProviderBinding {
    Concrete(Path),
    Dyn { provider: Path, trait_ty: Box<Type> },
}

impl Parse for ProviderBinding {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let provider: Path = input.parse()?;
        if input.peek(Token![as]) {
            input.parse::<Token![as]>()?;
            let trait_ty: Type = input.parse()?;
            Ok(Self::Dyn {
                provider,
                trait_ty: Box::new(trait_ty),
            })
        } else {
            Ok(Self::Concrete(provider))
        }
    }
}

impl Parse for ModuleArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut args = ModuleArgs::default();
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let content;
            bracketed!(content in input);

            match key.to_string().as_str() {
                "imports" => {
                    let paths: Punctuated<Path, Token![,]> =
                        Punctuated::parse_terminated(&content)?;
                    args.imports.extend(paths);
                }
                "providers" => {
                    let bindings: Punctuated<ProviderBinding, Token![,]> =
                        Punctuated::parse_terminated(&content)?;
                    args.providers.extend(bindings);
                }
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!(
                            "unknown #[module] key `{other}` (expected `imports` or `providers`)"
                        ),
                    ));
                }
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(args)
    }
}

// -----------------------------------------------------------------------------
// #[controller(path = "...")]
// -----------------------------------------------------------------------------

/// `#[controller(path = "/health")]` — paired with `#[routes]` on the impl block.
///
/// Generates a `from_container(&Container) -> Self` constructor and a
/// `pub const PATH: &'static str` used by `#[routes]` as the route prefix.
///
/// The `Discoverable` impl is emitted by `#[routes]` rather than here — it
/// needs the route table that `#[routes]` collects, and emitting it in two
/// places would conflict.
#[proc_macro_attribute]
pub fn controller(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as ControllerArgs);
    let mut item = parse_macro_input!(input as ItemStruct);

    let body = match build_injectable_body(&mut item) {
        Ok(body) => body,
        Err(err) => return err.to_compile_error().into(),
    };

    let name = item.ident.clone();
    let (impl_generics, ty_generics, where_clause) = item.generics.split_for_impl();
    let path_lit = args.path;

    quote! {
        #item

        impl #impl_generics #name #ty_generics #where_clause {
            pub const PATH: &'static str = #path_lit;

            pub fn from_container(container: &::nestrs_core::Container) -> Self {
                let _ = container;
                #body
            }
        }
    }
    .into()
}

struct ControllerArgs {
    path: LitStr,
}

impl Parse for ControllerArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let key: Ident = input.parse()?;
        if key != "path" {
            return Err(syn::Error::new(
                key.span(),
                "expected `path = \"...\"` as the only #[controller] argument",
            ));
        }
        input.parse::<Token![=]>()?;
        let path: LitStr = input.parse()?;
        Ok(ControllerArgs { path })
    }
}

// -----------------------------------------------------------------------------
// #[interceptor]
// -----------------------------------------------------------------------------

/// Mark a struct as an HTTP interceptor that the framework will discover
/// and wrap around the route tree.
///
/// Behaves like `#[injectable]` for construction (fields with `#[inject]`
/// pulled from the container, others default), and additionally emits an
/// `impl Discoverable` that attaches an `HttpInterceptorMeta` describing
/// this type. The HTTP transport reads those metas via
/// `DiscoveryService::meta::<HttpInterceptorMeta>()` at boot.
///
/// The struct must implement `nestrs_middleware::Interceptor` — the macro
/// emits an `Arc<dyn Interceptor>` cast that fails at compile time if it
/// does not.
#[proc_macro_attribute]
pub fn interceptor(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut item = parse_macro_input!(input as ItemStruct);

    let body = match build_injectable_body(&mut item) {
        Ok(body) => body,
        Err(err) => return err.to_compile_error().into(),
    };

    let name = item.ident.clone();
    let (impl_generics, ty_generics, where_clause) = item.generics.split_for_impl();

    quote! {
        #item

        impl #impl_generics #name #ty_generics #where_clause {
            pub fn from_container(container: &::nestrs_core::Container) -> Self {
                let _ = container;
                #body
            }
        }

        impl #impl_generics ::nestrs_core::Discoverable for #name #ty_generics #where_clause {
            fn register(
                builder: ::nestrs_core::ContainerBuilder,
            ) -> ::nestrs_core::ContainerBuilder {
                let __snapshot = builder.snapshot();
                let __value = Self::from_container(&__snapshot);
                let __arc: ::std::sync::Arc<dyn ::nestrs_middleware::Interceptor> =
                    ::std::sync::Arc::new(__value);
                builder.attach_meta::<Self, ::nestrs_http::HttpInterceptorMeta>(
                    ::nestrs_http::HttpInterceptorMeta::new(__arc),
                )
            }
        }
    }
    .into()
}

// -----------------------------------------------------------------------------
// #[routes]
// -----------------------------------------------------------------------------

/// Bind controller methods to HTTP routes.
///
/// Applied to an `impl` block belonging to a `#[controller]`-marked struct.
/// Each method tagged with `#[get("/path")]`, `#[post("/path")]`, `#[put]`,
/// `#[delete]` or `#[patch]` is wired as a poem handler. Method signatures
/// keep `&self` plus any poem extractors (`Path<T>`, `Json<T>`, `Query<T>`...).
///
/// Emits two impls on the controller:
/// - `nestrs_http::Controller` — the mount entry point used by the HTTP
///   transport.
/// - `nestrs_core::Discoverable` — attaches an `HttpControllerMeta` that
///   carries the declarative route table (verb + path + handler name) plus
///   a closure capturing the typed mount logic. The transport iterates
///   these metas at boot.
#[proc_macro_attribute]
pub fn routes(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut item = parse_macro_input!(input as ItemImpl);
    let self_ty = item.self_ty.clone();

    let mut wrappers: Vec<TokenStream2> = Vec::new();
    let mut route_entries: Vec<TokenStream2> = Vec::new();
    let mut route_metas: Vec<TokenStream2> = Vec::new();

    for impl_item in item.items.iter_mut() {
        let ImplItem::Fn(method) = impl_item else {
            continue;
        };

        let verb_idx = method.attrs.iter().position(|attr| {
            ["get", "post", "put", "delete", "patch"]
                .iter()
                .any(|v| attr.path().is_ident(v))
        });
        let Some(idx) = verb_idx else { continue };

        let attr = method.attrs.remove(idx);
        let verb_ident = attr
            .path()
            .get_ident()
            .expect("verb attribute has an ident")
            .clone();

        let route_path: LitStr = match attr.parse_args() {
            Ok(p) => p,
            Err(err) => return err.to_compile_error().into(),
        };

        let method_name = method.sig.ident.clone();
        let method_name_lit = method_name.to_string();
        let wrapper_name = format_ident!("__nestrs_route_{}", method_name);

        let inputs: Vec<FnArg> = method.sig.inputs.iter().skip(1).cloned().collect();
        let arg_idents: Vec<Ident> = inputs
            .iter()
            .filter_map(|arg| match arg {
                FnArg::Typed(pt) => match &*pt.pat {
                    Pat::Ident(pi) => Some(pi.ident.clone()),
                    _ => None,
                },
                _ => None,
            })
            .collect();

        let return_type = match &method.sig.output {
            ReturnType::Default => quote! { () },
            ReturnType::Type(_, ty) => quote! { #ty },
        };

        let extra_inputs = if inputs.is_empty() {
            quote! {}
        } else {
            quote! { , #(#inputs),* }
        };

        wrappers.push(quote! {
            #[::poem::handler]
            async fn #wrapper_name(
                ::poem::web::Data(__ctrl): ::poem::web::Data<&::std::sync::Arc<#self_ty>>
                #extra_inputs
            ) -> #return_type {
                __ctrl.#method_name(#(#arg_idents),*).await
            }
        });

        route_entries.push(quote! {
            .at(#route_path, ::poem::#verb_ident(#wrapper_name))
        });

        let verb_variant = match verb_ident.to_string().as_str() {
            "get" => quote!(::nestrs_http::HttpVerb::Get),
            "post" => quote!(::nestrs_http::HttpVerb::Post),
            "put" => quote!(::nestrs_http::HttpVerb::Put),
            "delete" => quote!(::nestrs_http::HttpVerb::Delete),
            "patch" => quote!(::nestrs_http::HttpVerb::Patch),
            _ => unreachable!("verb_ident filtered above"),
        };

        route_metas.push(quote! {
            ::nestrs_http::HttpRouteMeta {
                verb: #verb_variant,
                path: #route_path,
                handler: #method_name_lit,
            }
        });
    }

    quote! {
        #item

        #(#wrappers)*

        impl ::nestrs_http::Controller for #self_ty {
            fn mount(
                container: &::nestrs_core::Container,
                route: ::poem::Route,
            ) -> ::poem::Route {
                use ::poem::EndpointExt;
                let __ctrl = ::std::sync::Arc::new(<#self_ty>::from_container(container));
                let __sub = ::poem::Route::new()
                    #(#route_entries)*
                    .data(__ctrl);
                route.nest(<#self_ty>::PATH, __sub)
            }
        }

        impl ::nestrs_core::Discoverable for #self_ty {
            fn register(
                builder: ::nestrs_core::ContainerBuilder,
            ) -> ::nestrs_core::ContainerBuilder {
                let __mount: ::std::sync::Arc<
                    dyn ::core::ops::Fn(
                            &::nestrs_core::Container,
                            ::poem::Route,
                        ) -> ::poem::Route
                        + ::core::marker::Send
                        + ::core::marker::Sync,
                > = ::std::sync::Arc::new(|__c, __r| {
                    <#self_ty as ::nestrs_http::Controller>::mount(__c, __r)
                });
                let __meta = ::nestrs_http::HttpControllerMeta::new(
                    <#self_ty>::PATH,
                    ::std::vec![#(#route_metas),*],
                    __mount,
                );
                builder.attach_meta::<#self_ty, ::nestrs_http::HttpControllerMeta>(__meta)
            }
        }
    }
    .into()
}

// -----------------------------------------------------------------------------
// #[resolver(kind = Query | Mutation | Subscription)]
// -----------------------------------------------------------------------------

/// Mark a struct as a GraphQL resolver that participates in discovery.
///
/// Behaves like `#[injectable]` for construction (fields with `#[inject]`
/// resolved from the container, others default), and additionally emits
/// an `impl Discoverable` that attaches a `GraphQLResolverMeta` carrying
/// the resolver kind (Query / Mutation / Subscription).
///
/// Unlike controllers, the meta is informational — async-graphql composes
/// its `Schema<Q, M, S>` statically, so the schema is assembled by the
/// `#[graphql_app]` macro that names the resolvers explicitly. The meta
/// lets introspection tools list resolvers without parsing the schema.
///
/// Resolvers are *not* registered as providers in the container — they
/// are built fresh by the `#[graphql_app]` macro at schema-build time.
#[proc_macro_attribute]
pub fn resolver(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as ResolverArgs);
    let mut item = parse_macro_input!(input as ItemStruct);

    let body = match build_injectable_body(&mut item) {
        Ok(body) => body,
        Err(err) => return err.to_compile_error().into(),
    };

    let name = item.ident.clone();
    let (impl_generics, ty_generics, where_clause) = item.generics.split_for_impl();
    let kind_variant = match args.kind.as_str() {
        "Query" => quote!(::nestrs_graphql::ResolverKind::Query),
        "Mutation" => quote!(::nestrs_graphql::ResolverKind::Mutation),
        "Subscription" => quote!(::nestrs_graphql::ResolverKind::Subscription),
        _ => unreachable!("ResolverArgs::parse guards the variants"),
    };

    quote! {
        #item

        impl #impl_generics #name #ty_generics #where_clause {
            pub fn from_container(container: &::nestrs_core::Container) -> Self {
                let _ = container;
                #body
            }
        }

        impl #impl_generics ::nestrs_core::Discoverable for #name #ty_generics #where_clause {
            fn register(
                builder: ::nestrs_core::ContainerBuilder,
            ) -> ::nestrs_core::ContainerBuilder {
                builder.attach_meta::<Self, ::nestrs_graphql::GraphQLResolverMeta>(
                    ::nestrs_graphql::GraphQLResolverMeta::new(
                        #kind_variant,
                        ::core::any::TypeId::of::<Self>(),
                    ),
                )
            }
        }
    }
    .into()
}

struct ResolverArgs {
    kind: String,
}

impl Parse for ResolverArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let key: Ident = input.parse()?;
        if key != "kind" {
            return Err(syn::Error::new(
                key.span(),
                "expected `kind = Query | Mutation | Subscription` as the #[resolver] argument",
            ));
        }
        input.parse::<Token![=]>()?;
        let val: Ident = input.parse()?;
        let s = val.to_string();
        if !matches!(s.as_str(), "Query" | "Mutation" | "Subscription") {
            return Err(syn::Error::new(
                val.span(),
                "kind must be one of `Query`, `Mutation`, or `Subscription`",
            ));
        }
        Ok(Self { kind: s })
    }
}

// -----------------------------------------------------------------------------
// #[graphql_app(queries = [...], mutations = [...], subscriptions = [...])]
// -----------------------------------------------------------------------------

/// Compose discovered resolvers into a single GraphQL schema.
///
/// Applied to a unit marker struct. The macro generates one
/// `async_graphql::MergedObject` per root (Query / Mutation /
/// Subscription) and an inherent `build(container) -> Schema<...>`
/// method that constructs each resolver via `from_container` and
/// assembles the schema. The container is also attached as schema data.
///
/// Composition is static because `async-graphql`'s root types live in
/// the `Schema<Q, M, S>` parameters — they cannot be assembled
/// dynamically at runtime from a `DiscoveryService` walk. `#[resolver]`
/// still emits discovery metadata, so introspection / docs tooling can
/// list resolvers; only the schema wiring itself is static.
///
/// `queries = [...]` is required (a GraphQL schema needs a Query root).
/// `mutations` and `subscriptions` are optional — when omitted or empty,
/// async-graphql's `EmptyMutation` / `EmptySubscription` are used.
#[proc_macro_attribute]
pub fn graphql_app(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as GraphQLAppArgs);
    let item = parse_macro_input!(input as ItemStruct);
    let name = item.ident.clone();

    if args.queries.is_empty() {
        return syn::Error::new_spanned(
            &item,
            "#[graphql_app] requires a non-empty `queries = [...]` list",
        )
        .to_compile_error()
        .into();
    }

    let query_root = format_ident!("__{}QueryRoot", name);
    let mutation_root = format_ident!("__{}MutationRoot", name);
    let subscription_root = format_ident!("__{}SubscriptionRoot", name);

    let queries = &args.queries;
    let mutations = &args.mutations;
    let subscriptions = &args.subscriptions;

    let query_decl = quote! {
        #[derive(::nestrs_graphql::async_graphql::MergedObject)]
        pub struct #query_root(#(pub #queries),*);
    };
    let query_expr = quote! {
        #query_root(#(<#queries>::from_container(&container)),*)
    };

    let (mutation_decl, mutation_ty, mutation_expr) = if mutations.is_empty() {
        (
            quote!(),
            quote!(::nestrs_graphql::async_graphql::EmptyMutation),
            quote!(::nestrs_graphql::async_graphql::EmptyMutation),
        )
    } else {
        (
            quote! {
                #[derive(::nestrs_graphql::async_graphql::MergedObject)]
                pub struct #mutation_root(#(pub #mutations),*);
            },
            quote!(#mutation_root),
            quote! {
                #mutation_root(#(<#mutations>::from_container(&container)),*)
            },
        )
    };

    let (subscription_decl, subscription_ty, subscription_expr) = if subscriptions.is_empty() {
        (
            quote!(),
            quote!(::nestrs_graphql::async_graphql::EmptySubscription),
            quote!(::nestrs_graphql::async_graphql::EmptySubscription),
        )
    } else {
        (
            quote! {
                #[derive(::nestrs_graphql::async_graphql::MergedSubscription)]
                pub struct #subscription_root(#(pub #subscriptions),*);
            },
            quote!(#subscription_root),
            quote! {
                #subscription_root(#(<#subscriptions>::from_container(&container)),*)
            },
        )
    };

    quote! {
        #item

        #query_decl
        #mutation_decl
        #subscription_decl

        impl #name {
            pub fn build(
                container: ::nestrs_core::Container,
            ) -> ::nestrs_graphql::async_graphql::Schema<
                #query_root,
                #mutation_ty,
                #subscription_ty,
            > {
                ::nestrs_graphql::async_graphql::Schema::build(
                    #query_expr,
                    #mutation_expr,
                    #subscription_expr,
                )
                .data(container)
                .finish()
            }
        }
    }
    .into()
}

#[derive(Default)]
struct GraphQLAppArgs {
    queries: Vec<Path>,
    mutations: Vec<Path>,
    subscriptions: Vec<Path>,
}

impl Parse for GraphQLAppArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut args = GraphQLAppArgs::default();
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let content;
            bracketed!(content in input);
            let paths: Punctuated<Path, Token![,]> = Punctuated::parse_terminated(&content)?;

            match key.to_string().as_str() {
                "queries" => args.queries.extend(paths),
                "mutations" => args.mutations.extend(paths),
                "subscriptions" => args.subscriptions.extend(paths),
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!(
                            "unknown #[graphql_app] key `{other}` (expected `queries`, `mutations`, or `subscriptions`)"
                        ),
                    ));
                }
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(args)
    }
}
