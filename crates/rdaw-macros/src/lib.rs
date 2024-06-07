use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use proc_macro_error::{emit_error, proc_macro_error};
use quote::{format_ident, quote, quote_spanned};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::*;

#[proc_macro_attribute]
#[proc_macro_error]
pub fn api_operations(args: TokenStream, item: TokenStream) -> TokenStream {
    let ItemTrait {
        attrs,
        vis,
        unsafety,
        ident,
        generics,
        mut supertraits,
        items,
        ..
    } = parse_macro_input!(item as ItemTrait);

    let protocol_path = parse_macro_input!(args as Path);

    let mut funcs = items
        .iter()
        .flat_map(|item| match item {
            TraitItem::Fn(v) => Some(v.clone()),
            _ => {
                emit_error!(item.span(), "items other than `fn` are not supported");
                None
            }
        })
        .collect::<Vec<_>>();

    let ident_without_ops = Ident::new(
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
        let orig_func_sig = func.sig.clone();

        let mut is_sub = false;
        func.attrs.retain(|attr| {
            is_sub = attr.path().is_ident("sub");
            !is_sub
        });

        let ReturnType::Type(_, func_ret_ty_res) = &func.sig.output else {
            emit_error!(func.sig.output.span(), "method must return `Result<T>`");
            continue;
        };

        let Some(func_ret_ty) = unwrap_type(func_ret_ty_res, "Result") else {
            emit_error!(func.sig.output.span(), "method must return `Result<T>`");
            continue;
        };

        let variant_span = func.sig.ident.span();
        let variant_ident = Ident::new(
            &func.sig.ident.to_string().to_case(Case::Pascal),
            variant_span,
        );

        let params = func.sig.inputs.iter().flat_map(|arg| match arg {
            FnArg::Typed(arg) => Some(arg),
            _ => None,
        });

        let mut req_variant_fields = Vec::new();
        let mut param_names = Vec::new();

        for param in params {
            let Pat::Ident(pat) = &*param.pat else {
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
            let replacement = Type::Verbatim(quote_spanned! { func_ret_ty_res.span() =>
                crate::EventStreamId
            });

            let Some((orig_ok_ty, result_ty)) =
                replace_result_ok_type(func_ret_ty_res, replacement)
            else {
                emit_error!(
                    func.sig.output.span(),
                    "method marked with `#[subscribe]` must return a `Result<BoxStream<T>>`"
                );
                continue;
            };

            let Some(event_ty) = unwrap_type(&orig_ok_ty, "BoxStream") else {
                emit_error!(
                    func.sig.output.span(),
                    "method marked with `#[subscribe]` must return a `Result<BoxStream<T>>`"
                );
                continue;
            };

            let res_variant = quote_spanned!(variant_span => #variant_ident(#result_ty));
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
            impl ::core::future::Future<Output = #func_ret_ty_res> + Send
        };

        func.sig.output = ReturnType::Type(
            Token![->](new_output_ty.span()),
            Box::new(Type::Verbatim(new_output_ty)),
        );

        let func_body = if is_sub {
            quote! {
                use futures_lite::StreamExt;
                let res = self.request(
                    #req_enum_ident::#variant_ident { #(#param_names,)* }.into()
                ).await?;
                let res: #res_enum_ident  = res.try_into().map_err(|_| crate::Error::InvalidType)?;
                let id = match res {
                    #res_enum_ident::#variant_ident(v) => v?,
                    _ => return Err(crate::Error::InvalidType),
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
                let res: #res_enum_ident  = res.try_into().map_err(|_| crate::Error::InvalidType)?;
                match res {
                    #res_enum_ident::#variant_ident(v) => Ok(v),
                    _ => Err(crate::Error::InvalidType),
                }
            }
        };

        let func_impl = quote! {
            #[allow(unused_variables)]
            #orig_func_sig {
                #func_body
            }
        };

        func_impls.push(func_impl);
    }

    supertraits.push(TypeParamBound::Verbatim(quote! { Send }));

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
        impl<T> #ident for crate::Client<#protocol_path, T>
        where
            T: crate::transport::ClientTransport<#protocol_path>
        {
            #(#func_impls)*
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
#[proc_macro_error]
pub fn api_protocol(args: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemStruct);
    let vis = item.vis.clone();

    let ops_traits = parse_macro_input!(args as PathList)
        .paths
        .into_iter()
        .collect::<Vec<_>>();

    let ops_names = ops_traits
        .iter()
        .map(|v| {
            let ident = &v.segments.last().unwrap().ident;
            Ident::new(
                ident.to_string().trim_end_matches("Operations"),
                ident.span(),
            )
        })
        .collect::<Vec<_>>();

    let ident = &item.ident;
    let ident_prefix = Ident::new(ident.to_string().trim_end_matches("Protocol"), ident.span());

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

        #[automatically_derived]
        impl crate::Protocol for #ident {
            type Req = #req_enum_ident;
            type Res = #res_enum_ident;
            type Event = #event_enum_ident;
        }
    };

    TokenStream::from(expanded)
}

struct PathList {
    paths: Punctuated<Path, Token![,]>,
}

impl syn::parse::Parse for PathList {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let paths = Punctuated::parse_terminated(input)?;
        Ok(PathList { paths })
    }
}

fn generate_sum_enum(
    vis: &Visibility,
    prefix: &Ident,
    paths: &[Path],
    idents: &[Ident],
    suffix: &str,
) -> (Ident, proc_macro2::TokenStream) {
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

fn replace_result_ok_type(ty: &Type, new_ok: Type) -> Option<(Type, Type)> {
    let Type::Path(mut ret_ty) = ty.clone() else {
        return None;
    };

    let last_seg = ret_ty.path.segments.last_mut()?;

    if &last_seg.ident.to_string() != "Result" {
        return None;
    }

    let PathArguments::AngleBracketed(args) = &mut last_seg.arguments else {
        return None;
    };

    let GenericArgument::Type(orig_ty) = args.args.iter_mut().next()? else {
        return None;
    };

    let orig_ty = std::mem::replace(orig_ty, new_ok);

    Some((orig_ty, ret_ty.into()))
}

fn unwrap_type<'a>(ty: &'a Type, expect: &str) -> Option<&'a Type> {
    let Type::Path(ret_ty) = ty else {
        return None;
    };

    let last_seg = ret_ty.path.segments.last()?;

    if &last_seg.ident.to_string() != expect {
        return None;
    }

    let PathArguments::AngleBracketed(args) = &last_seg.arguments else {
        return None;
    };

    let GenericArgument::Type(orig_ty) = args.args.iter().next().unwrap() else {
        return None;
    };

    Some(orig_ty)
}
