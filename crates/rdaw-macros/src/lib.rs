use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use proc_macro_error::{emit_error, proc_macro_error};
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;
use syn::*;

#[proc_macro_attribute]
#[proc_macro_error]
pub fn api_operations(_args: TokenStream, item: TokenStream) -> TokenStream {
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

    let mut funcs = items
        .iter()
        .flat_map(|item| match item {
            TraitItem::Fn(v) => Some(v.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();

    let other_items = items
        .iter()
        .flat_map(|item| match item {
            TraitItem::Fn(_) => None,
            _ => Some(item),
        })
        .collect::<Vec<_>>();

    let req_enum_ident = format_ident!("{ident}Request");
    let res_enum_ident = format_ident!("{ident}Response");
    let event_enum_ident = format_ident!("{ident}Event");

    let mut req_enum_variants = Vec::new();
    let mut res_enum_variants = Vec::new();
    let mut event_enum_variants = Vec::new();

    for func in &mut funcs {
        let mut is_sub = false;
        func.attrs.retain(|attr| {
            is_sub = attr.path().is_ident("sub");
            !is_sub
        });

        let variant_span = func.sig.ident.span();
        let variant_ident = Ident::new(
            &func.sig.ident.to_string().to_case(Case::Pascal),
            variant_span,
        );

        let params = func.sig.inputs.iter().flat_map(|arg| match arg {
            FnArg::Typed(arg) => Some(arg),
            _ => None,
        });

        let client_variant_fields = params.flat_map(|param| {
            let Pat::Ident(pat) = &*param.pat else {
                emit_error!(param.pat, "complex patterns in arguments are not supported");
                return None;
            };

            let ident = &pat.ident;
            let ty = &param.ty;
            Some(quote_spanned!(variant_span => #ident: #ty))
        });

        let req_variant = quote_spanned! { variant_span =>
            #variant_ident {
                #(#client_variant_fields,)*
            }
        };

        let (res_variant, event_variant) = if is_sub {
            let ReturnType::Type(_, ret_ty) = &func.sig.output else {
                emit_error!(
                    func.sig.output.span(),
                    "method marked with `#[subscribe]` must return a `Result<BoxStream<T>, E>`"
                );
                continue;
            };

            let replacement = Type::Verbatim(quote_spanned! { ret_ty.span() =>
                crate::EventStreamId
            });

            let Some((orig_ok_ty, result_ty)) = replace_result_ok_type(ret_ty, replacement) else {
                emit_error!(
                    func.sig.output.span(),
                    "method marked with `#[subscribe]` must return a `Result<BoxStream<T>, E>`"
                );
                continue;
            };

            let Some(event_ty) = unwrap_event_type(&orig_ok_ty) else {
                emit_error!(
                    func.sig.output.span(),
                    "method marked with `#[subscribe]` must return a `Result<BoxStream<T>, E>`"
                );
                continue;
            };

            let res_variant = quote_spanned!(variant_span => #variant_ident(#result_ty));
            let event_variant = quote_spanned!(variant_span => #variant_ident(#event_ty));

            (res_variant, Some(event_variant))
        } else {
            let res_variant = match &func.sig.output {
                ReturnType::Default => quote!(#variant_ident),
                ReturnType::Type(_, ty) => quote!(#variant_ident(#ty)),
            };

            (res_variant, None)
        };

        req_enum_variants.push(req_variant);
        res_enum_variants.push(res_variant);
        event_enum_variants.extend(event_variant);

        if func.sig.asyncness.is_some() {
            func.sig.asyncness = None;

            let output_ty = match &func.sig.output {
                ReturnType::Default => quote_spanned!(func.sig.paren_token.span => ()),
                ReturnType::Type(_, ty) => quote!(#ty),
            };

            let new_output_ty = quote_spanned! { output_ty.span() =>
                impl ::core::future::Future<Output = #output_ty> + Send
            };

            func.sig.output = ReturnType::Type(
                Token![->](new_output_ty.span()),
                Box::new(Type::Verbatim(new_output_ty)),
            );
        } else {
            emit_error!(func.sig, "method must be async");
        }
    }

    supertraits.push(TypeParamBound::Verbatim(quote! { Send }));

    let expanded = quote! {
        #(#attrs)*
        #vis #unsafety trait #ident #generics: #supertraits {
            #(#funcs)*
            #(#other_items)*
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
    };

    TokenStream::from(expanded)
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

fn unwrap_event_type(ty: &Type) -> Option<&Type> {
    let Type::Path(ret_ty) = ty else {
        return None;
    };

    let last_seg = ret_ty.path.segments.last()?;

    if &last_seg.ident.to_string() != "BoxStream" {
        return None;
    }

    let PathArguments::AngleBracketed(args) = &last_seg.arguments else {
        return None;
    };

    if args.args.len() != 1 {
        return None;
    }

    let GenericArgument::Type(orig_ty) = args.args.iter().next().unwrap() else {
        return None;
    };

    Some(orig_ty)
}
