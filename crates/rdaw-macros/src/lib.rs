use convert_case::{Case, Casing};
use darling::ast::NestedMeta;
use darling::util::PathList;
use darling::FromMeta;
use proc_macro::TokenStream;
use proc_macro_error::{emit_error, proc_macro_error};
use quote::{format_ident, quote, quote_spanned, ToTokens};
use syn::spanned::Spanned;

#[derive(Debug, FromMeta)]
struct ApiOperationsArgs {
    protocol: syn::Path,
}

#[proc_macro_attribute]
#[proc_macro_error]
pub fn rpc_operations(args: TokenStream, item: TokenStream) -> TokenStream {
    let syn::ItemTrait {
        attrs,
        vis,
        unsafety,
        ident,
        generics,
        mut supertraits,
        items,
        ..
    } = syn::parse_macro_input!(item as syn::ItemTrait);

    let args = match parse_macro_args::<ApiOperationsArgs>(args) {
        Ok(v) => v,
        Err(e) => return e.write_errors().into(),
    };

    let protocol_path = args.protocol;

    let error_path = quote!(<#protocol_path as rdaw_rpc::Protocol>::Error);
    let error_path_as = quote!(<#error_path as rdaw_rpc::ProtocolError>);

    let mut funcs = items
        .iter()
        .flat_map(|item| match item {
            syn::TraitItem::Fn(v) => Some(v.clone()),
            _ => {
                emit_error!(item, "items other than `fn` are not supported");
                None
            }
        })
        .collect::<Vec<_>>();

    let ident_without_ops = syn::Ident::new(
        ident.to_string().trim_end_matches("Operations"),
        ident.span(),
    );

    let req_enum_ident = format_ident!("{ident_without_ops}Request");
    let res_enum_ident = format_ident!("{ident_without_ops}Response");
    let event_enum_ident = format_ident!("{ident_without_ops}Events");

    let mut req_enum_variants = Vec::new();
    let mut res_enum_variants = Vec::new();
    let mut event_enum_variants = Vec::new();
    let mut func_impls = Vec::new();

    for func in &mut funcs {
        let mut is_sub = false;
        func.attrs.retain(|attr| {
            is_sub = attr.path().is_ident("sub");
            !is_sub
        });

        let syn::ReturnType::Type(_, func_ret_ty_res) = &func.sig.output else {
            emit_error!(func.sig.output, "method must return `Result<T>`");
            continue;
        };

        let Some(func_ret_ty) = unwrap_type(func_ret_ty_res, "Result") else {
            emit_error!(func.sig.output, "method must return `Result<T>`");
            continue;
        };

        let variant_span = func.sig.ident.span();
        let variant_ident = syn::Ident::new(
            &func.sig.ident.to_string().to_case(Case::Pascal),
            variant_span,
        );

        let params = func.sig.inputs.iter().flat_map(|arg| match arg {
            syn::FnArg::Typed(arg) => Some(arg),
            _ => None,
        });

        let mut req_variant_fields = Vec::new();
        let mut param_names = Vec::new();

        for param in params {
            let syn::Pat::Ident(pat) = &*param.pat else {
                emit_error!(param.pat, "complex patterns in arguments are not supported");
                continue;
            };

            let ident = &pat.ident;
            let ty = &param.ty;

            req_variant_fields.push(quote!(#ident: #ty));
            param_names.push(ident);
        }

        let req_variant = quote_spanned! { variant_span =>
            #variant_ident {
                #(#req_variant_fields,)*
            }
        };

        let (res_variant, event_variant) = if is_sub {
            let replacement = syn::Type::Verbatim(quote_spanned! { func_ret_ty_res.span() =>
                rdaw_rpc::StreamId
            });

            let Some((orig_ok_ty, _)) = replace_result_ok_type(func_ret_ty_res, replacement) else {
                emit_error!(
                    func.sig.output,
                    "method marked with `#[subscribe]` must return a `Result<BoxStream<T>>`"
                );
                continue;
            };

            let Some(event_ty) = unwrap_type(&orig_ok_ty, "BoxStream") else {
                emit_error!(
                    func.sig.output,
                    "method marked with `#[subscribe]` must return a `Result<BoxStream<T>>`"
                );
                continue;
            };

            let res_variant = quote_spanned!(variant_span => #variant_ident(rdaw_rpc::StreamId));
            let event_variant = quote_spanned!(variant_span => #variant_ident(#event_ty));

            (res_variant, Some(event_variant))
        } else {
            let res_variant = quote!(#variant_ident(#func_ret_ty));
            (res_variant, None)
        };

        req_enum_variants.push(req_variant);
        res_enum_variants.push(res_variant);
        event_enum_variants.extend(event_variant);

        if func.sig.asyncness.is_none() {
            emit_error!(func.sig, "method must be async");
            continue;
        }

        func.sig.asyncness = None;

        let new_output_ty = quote_spanned! { func_ret_ty_res.span() =>
            std::pin::Pin<Box<dyn std::future::Future<Output = #func_ret_ty_res> + Send + '_>>
        };

        func.sig.output = syn::ReturnType::Type(
            syn::Token![->](new_output_ty.span()),
            Box::new(syn::Type::Verbatim(new_output_ty)),
        );

        let func_body = if is_sub {
            quote! {
                use futures::StreamExt as _;

                let res = self.request(
                    #req_enum_ident::#variant_ident { #(#param_names,)* }.into()
                ).await?;

                let res: #res_enum_ident = res
                    .try_into()
                    .map_err(|_| #error_path_as::invalid_type())?;

                let id = match res {
                    #res_enum_ident::#variant_ident(v) => v,
                    _ => return Err(#error_path_as::invalid_type()),
                };

                let stream = self.subscribe(id)
                    .map(|v| {
                        let ev: #event_enum_ident = v.try_into().ok().unwrap();
                        match ev {
                            #event_enum_ident::#variant_ident(v) => v,
                            _ => panic!(),
                        }
                    })
                    .boxed();
                Ok(stream)
            }
        } else {
            quote! {
                let res = self.request(
                    #req_enum_ident::#variant_ident { #(#param_names,)* }.into()
                ).await?;

                let res: #res_enum_ident = res
                    .try_into()
                    .map_err(|_| #error_path_as::invalid_type())?;

                match res {
                    #res_enum_ident::#variant_ident(v) => Ok(v),
                    _ => Err(#error_path_as::invalid_type()),
                }
            }
        };

        let func_sig = &func.sig;

        let func_impl = quote! {
            #[allow(unused_variables)]
            #func_sig {
                Box::pin(async move {
                    #func_body
                })
            }
        };

        func_impls.push(func_impl);
    }

    supertraits.push(syn::TypeParamBound::Verbatim(quote! { Send }));

    let expanded = quote! {
        #(#attrs)*
        #vis #unsafety trait #ident #generics: #supertraits {
            #(#funcs)*
        }

        #[derive(Debug, Clone)]
        #vis enum #req_enum_ident {
            #(#req_enum_variants,)*
        }

        #[derive(Debug, Clone)]
        #vis enum #res_enum_ident {
            #(#res_enum_variants,)*
        }

        #[derive(Debug, Clone)]
        #vis enum #event_enum_ident {
            #(#event_enum_variants,)*
        }

        #[automatically_derived]
        impl<T> #ident for rdaw_rpc::Client<#protocol_path, T>
        where
            T: rdaw_rpc::transport::ClientTransport<#protocol_path>
        {
            #(#func_impls)*
        }
    };

    TokenStream::from(expanded)
}

#[derive(Debug, FromMeta)]
struct ApiProtocolArgs {
    operations: PathList,
    error: syn::Path,
}

#[proc_macro_attribute]
#[proc_macro_error]
pub fn rpc_protocol(args: TokenStream, item: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item as syn::ItemStruct);
    let vis = item.vis.clone();

    let args = match parse_macro_args::<ApiProtocolArgs>(args) {
        Ok(v) => v,
        Err(e) => return e.write_errors().into(),
    };

    let ops_traits = args.operations;
    let error_path = args.error;

    let ops_names = ops_traits
        .iter()
        .map(|v| {
            let ident = &v.segments.last().unwrap().ident;
            syn::Ident::new(
                ident.to_string().trim_end_matches("Operations"),
                ident.span(),
            )
        })
        .collect::<Vec<_>>();

    let ident = &item.ident;
    let ident_prefix =
        syn::Ident::new(ident.to_string().trim_end_matches("Protocol"), ident.span());

    let (req_enum_ident, req_enum) =
        generate_sum_enum(&vis, &ident_prefix, &ops_traits, &ops_names, "Request");
    let (res_enum_ident, res_enum) =
        generate_sum_enum(&vis, &ident_prefix, &ops_traits, &ops_names, "Response");
    let (event_enum_ident, event_enum) =
        generate_sum_enum(&vis, &ident_prefix, &ops_traits, &ops_names, "Events");

    let expanded = quote! {
        #item
        #req_enum
        #res_enum
        #event_enum

        #vis trait #ident_prefix: 'static + Sync #(+ #ops_traits)* {}

        impl<T> #ident_prefix for T where T: 'static + Sync #(+ #ops_traits)* {}

        #[automatically_derived]
        impl rdaw_rpc::Protocol for #ident {
            type Req = #req_enum_ident;
            type Res = #res_enum_ident;
            type Event = #event_enum_ident;
            type Error = #error_path;
        }
    };

    TokenStream::from(expanded)
}

#[derive(Debug, FromMeta)]
struct ApiHandlerArgs {
    protocol: syn::Path,
    operations: syn::Path,
}

#[proc_macro_attribute]
#[proc_macro_error]
pub fn rpc_handler(args: TokenStream, item: TokenStream) -> TokenStream {
    let mut item = syn::parse_macro_input!(item as syn::ItemImpl);

    let args = match parse_macro_args::<ApiHandlerArgs>(args) {
        Ok(v) => v,
        Err(e) => return e.write_errors().into(),
    };

    let protocol_path = args.protocol;
    let ops_path = args.operations;

    let error_path = quote!(<#protocol_path as rdaw_rpc::Protocol>::Error);

    let ident = &ops_path.segments.last().unwrap().ident;
    let ident_str = ident.to_string();
    let base = ident_str.trim_end_matches("Operations");

    let req_ident = syn::Ident::new(&format!("{base}Request"), ident.span());
    let res_ident = syn::Ident::new(&format!("{base}Response"), ident.span());

    let method_name = syn::Ident::new(
        &format!("handle_{}_request", base.to_case(Case::Snake)),
        ident.span(),
    );

    let mut match_cases = Vec::new();

    for item in &mut item.items {
        let syn::ImplItem::Fn(func) = item else {
            continue;
        };

        let mut is_handler = false;

        func.attrs.retain(|attr| {
            let this_is_handler = attr.path().is_ident("handler");
            is_handler |= this_is_handler;
            !this_is_handler
        });

        if !is_handler {
            continue;
        }

        let func_name = &func.sig.ident;
        let name = syn::Ident::new(
            &func.sig.ident.to_string().to_case(Case::Pascal),
            func.sig.ident.span(),
        );

        let mut args = func
            .sig
            .inputs
            .iter()
            .flat_map(|v| match v {
                syn::FnArg::Receiver(_) => None,
                syn::FnArg::Typed(syn::PatType { pat, .. }) => match **pat {
                    syn::Pat::Ident(ref ident) => Some(ident.ident.clone()),
                    _ => {
                        emit_error!(pat, "complex patterns in arguments are not supported");
                        None
                    }
                },
            })
            .collect::<Vec<_>>();

        let has_responder = args.first().is_some_and(|arg| arg == "responder");

        let match_case = if has_responder {
            args.remove(0);
            quote! {
                #req_ident::#name { #(#args,)* } => {
                    let responder = rdaw_rpc::ClosureResponder::new(move |res: Result<_, #error_path>| {
                        let payload = res
                            .map(#res_ident::#name)
                            .map(|v| v.into());
                        async move {
                            transport
                                .send(rdaw_rpc::ServerMessage::Response { id: req_id, payload })
                                .await
                        }
                    });

                    self.#func_name(responder, #(#args,)*)
                }
            }
        } else {
            quote! {
                #req_ident::#name { #(#args,)* } => {
                    let payload = self
                        .#func_name(#(#args,)*)
                        .map(#res_ident::#name)
                        .map(|v| v.into());
                    transport
                        .send(rdaw_rpc::ServerMessage::Response { id: req_id, payload })
                        .await
                }
            }
        };

        match_cases.push(match_case);
    }

    let handler = quote! {
        pub async fn #method_name<T: rdaw_rpc::transport::ServerTransport<#protocol_path>>(
            &mut self,
            transport: T,
            req_id: rdaw_rpc::RequestId,
            req: #req_ident,
        ) -> Result<(), #error_path> {
            #[allow(dead_code)]
            fn _suppress<T: #ops_path>() {}

            match req {
                #(#match_cases)*
            }
        }
    };

    item.items.push(syn::ImplItem::Verbatim(handler));

    item.to_token_stream().into()
}

fn parse_macro_args<T: FromMeta>(args: TokenStream) -> Result<T, darling::Error> {
    let attr_args = NestedMeta::parse_meta_list(args.into())?;
    T::from_list(&attr_args)
}

fn generate_sum_enum(
    vis: &syn::Visibility,
    prefix: &syn::Ident,
    paths: &[syn::Path],
    idents: &[syn::Ident],
    suffix: &str,
) -> (syn::Ident, proc_macro2::TokenStream) {
    let enum_ident = format_ident!("{prefix}{suffix}");

    let mut enum_variants = Vec::new();
    let mut impls = Vec::new();

    for (path, ident) in paths.iter().zip(idents) {
        let mut path = path.clone();
        let seg = path.segments.last_mut().unwrap();
        seg.ident = format_ident!("{ident}{suffix}");

        enum_variants.push(quote!(#ident(#path)));
        impls.push(quote! {
            #[automatically_derived]
            impl From<#path> for #enum_ident {
                fn from(v: #path) -> #enum_ident {
                    #enum_ident::#ident(v)
                }
            }

            #[automatically_derived]
            impl TryFrom<#enum_ident> for #path {
                type Error = ();

                fn try_from(v: #enum_ident) -> Result<#path, ()> {
                    match v {
                        #enum_ident::#ident(v) => Ok(v),
                        _ => Err(())
                    }
                }
            }
        });
    }

    let tt = quote! {
        #[derive(Debug, Clone)]
        #vis enum #enum_ident{
            #(#enum_variants,)*
        }

        #(#impls)*
    };

    (enum_ident, tt)
}

fn replace_result_ok_type(ty: &syn::Type, new_ok: syn::Type) -> Option<(syn::Type, syn::Type)> {
    let syn::Type::Path(mut ret_ty) = ty.clone() else {
        return None;
    };

    let last_seg = ret_ty.path.segments.last_mut()?;

    if &last_seg.ident.to_string() != "Result" {
        return None;
    }

    let syn::PathArguments::AngleBracketed(args) = &mut last_seg.arguments else {
        return None;
    };

    let syn::GenericArgument::Type(orig_ty) = args.args.iter_mut().next()? else {
        return None;
    };

    let orig_ty = std::mem::replace(orig_ty, new_ok);

    Some((orig_ty, ret_ty.into()))
}

fn unwrap_type<'a>(ty: &'a syn::Type, expect: &str) -> Option<&'a syn::Type> {
    let syn::Type::Path(ret_ty) = ty else {
        return None;
    };

    let last_seg = ret_ty.path.segments.last()?;

    if last_seg.ident != expect {
        return None;
    }

    let syn::PathArguments::AngleBracketed(args) = &last_seg.arguments else {
        return None;
    };

    let syn::GenericArgument::Type(orig_ty) = args.args.iter().next().unwrap() else {
        return None;
    };

    Some(orig_ty)
}
