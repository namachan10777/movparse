use itertools::{Either, Itertools};
use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::{quote, ToTokens, TokenStreamExt};
use syn::{
    parse_macro_input,
    punctuated::Punctuated,
    spanned::Spanned,
    token::{Bracket, Colon2},
    Attribute, DeriveInput, Expr, ExprAssign, ExprPath, Fields, LitStr, Type,
};

fn acceptable_tag(attrs: &Vec<Attribute>, span: &Span) -> Result<syn::Expr, syn::Error> {
    let attrs = parse_mp4_attrs(attrs)?;
    let mut tags = Punctuated::new();
    for attr in attrs {
        if let Mp4Attr::WithValue(path, value) = attr {
            if path.is_ident("tag") {
                let syn::Lit::Str(s) = &value else {
                return Err(syn::Error::new_spanned(value.to_token_stream(), "mp4(tag = <4 char str>)"));
            };
                let tag_str = s.value();
                let [a, b, c, d] = tag_str.as_bytes() else {
                return Err(syn::Error::new_spanned(value.to_token_stream(), "mp4(tag = <4 char str>)"));
            };
                let mut tag_chars = Punctuated::new();
                for ch in [*a, *b, *c, *d] {
                    let ch = syn::Lit::Int(syn::LitInt::new(&format!("{}u8", ch), s.span()));
                    tag_chars.push(syn::Expr::Lit(syn::ExprLit {
                        attrs: Default::default(),
                        lit: ch,
                    }));
                }
                let tag = syn::Expr::Array(syn::ExprArray {
                    attrs: Default::default(),
                    bracket_token: Bracket::default(),
                    elems: tag_chars,
                });
                tags.push(tag);
            }
        }
    }

    if tags.is_empty() {
        return Err(syn::Error::new(
            *span,
            "at least 1 mp4(tag = <4 char str>) required",
        ));
    }

    let tags_def = syn::Expr::Array(syn::ExprArray {
        attrs: Vec::new(),
        bracket_token: Bracket::default(),
        elems: tags,
    });

    Ok(tags_def)
}

fn gen_read_leaf_struct_inner(
    name: &TokenStream2,
    fields: &Fields,
    span: Span,
) -> Result<TokenStream2, syn::Error> {
    let fields_info = parse_fields(fields, span)?;
    let (assign_stmts, struct_return) = match fields_info {
        FieldsInfo::Struct {
            header_name,
            fields,
        } => {
            let header_name = header_name.ok_or_else(|| {
                syn::Error::new(span, "At least one #[mp4(header)] attribute required")
            })?;

            let assign_stmts = fields.iter().map(|field| {
                let allocated_name = &field.allocated_name;
                quote!{
                    let #allocated_name = ::movparse_box::AttrRead::read_attr(&mut reader2).await?;
                }
            }).fold(TokenStream2::new(), |mut acc, stmt| {
                acc.append_all(stmt);
                acc
            });
            let struct_fills = fields
                .iter()
                .map(|field| {
                    let allocated_name = &field.allocated_name;
                    let field_name = &field.field_name;
                    quote! {
                        #field_name: #allocated_name,
                    }
                })
                .fold(quote! {#header_name: header,}, |mut acc, fill| {
                    acc.append_all(fill);
                    acc
                });
            (assign_stmts, quote! {Ok(#name{#struct_fills})})
        }
        FieldsInfo::Tuple { fields } => {
            let assign_stmts = fields.iter().flat_map(|field| {
                match field {
                    TupleFieldInfo::Header { allocated_name:_ } => None,
                    TupleFieldInfo::NormalField { ty: _, allocated_name }  => Some({
                        quote!{
                            let #allocated_name = ::movparse_box::AttrRead::read_attr(&mut reader2).await?;
                        }
                    })
                }
            }).fold(TokenStream2::new(), |mut acc, stmt| {
                acc.append_all(stmt);
                acc
            });
            let struct_fills = fields
                .iter()
                .map(|field| match field {
                    TupleFieldInfo::Header { allocated_name: _ } => quote! {header,},
                    TupleFieldInfo::NormalField {
                        ty: _,
                        allocated_name,
                    } => quote! {#allocated_name,},
                })
                .fold(TokenStream2::new(), |mut acc, fill| {
                    acc.append_all(fill);
                    acc
                });
            (assign_stmts, quote! {Ok(#name(#struct_fills))})
        }
    };

    let derived = quote! {
        let mut reader2 = reader.clone();
        reader2.set_limit(header.body_size() as u64);
        #assign_stmts
        reader.seek_from_current(header.body_size() as i64).await?;
        return #struct_return
    };
    Ok(derived)
}

fn gen_read_leaf_struct(
    name: &Ident,
    attrs: &Vec<Attribute>,
    strct: &syn::DataStruct,
) -> Result<TokenStream, syn::Error> {
    let inner = gen_read_leaf_struct_inner(
        &name.to_token_stream(),
        &strct.fields,
        strct.struct_token.span,
    )?;

    let acceptable_tag = acceptable_tag(attrs, &strct.struct_token.span)?;

    let derived = quote! {
        #[::async_trait::async_trait]
        impl ::movparse_box::BoxRead for #name {

            fn acceptable_tag(tag: [u8;4]) -> bool {
                #acceptable_tag.contains(&tag)
            }

            async fn read_body<R: ::tokio::io::AsyncRead + ::tokio::io::AsyncSeek + ::std::marker::Unpin + ::std::marker::Send>(
                header: ::movparse_box::BoxHeader,
                reader: &mut ::movparse_box::Reader<R>,
            ) -> std::io::Result<Self> {
                #inner
            }
        }
    };
    Ok(derived.into())
}

struct StructFieldInfo {
    ty: TokenStream2,
    field_name: Ident,
    field_name_str_lit: LitStr,
    allocated_name: Ident,
}

enum TupleFieldInfo {
    NormalField {
        ty: TokenStream2,
        allocated_name: Ident,
    },
    Header {
        allocated_name: Ident,
    },
}

enum FieldsInfo {
    Struct {
        header_name: Option<Ident>,
        fields: Vec<StructFieldInfo>,
    },
    Tuple {
        fields: Vec<TupleFieldInfo>,
    },
}

fn canonicalize_ty(ty: &Type) -> TokenStream2 {
    match ty {
        syn::Type::Path(path) => {
            let mut tokens = Punctuated::<_, Colon2>::new();
            for (idx, segment) in (&path.path.segments).into_iter().enumerate() {
                let ident = &segment.ident;
                if idx == 0 {
                    tokens.push(quote! {
                        #ident
                    });
                } else {
                    tokens.push(quote! {
                        ::#ident
                    });
                }
                if !segment.arguments.is_empty() {
                    let arguments = &segment.arguments;
                    tokens.push(quote! {
                        #arguments
                    });
                }
            }
            tokens.to_token_stream()
        }
        _ => quote! {
            <#ty>
        },
    }
}

fn parse_fields(fields: &Fields, span: Span) -> Result<FieldsInfo, syn::Error> {
    let Some(field_sample) = fields.iter().next() else {
        return Err(syn::Error::new(span, "movparse-derive requires C-style struct or tuple struct.".to_owned()));
    };
    if field_sample.ident.is_some() {
        let (headers, fields): (Vec<_>, Vec<_>) =
            fields.iter().enumerate().partition_map(|(idx, field)| {
                let Some(field_name) = field.ident.as_ref()else {
                return Either::Right(Err(syn::Error::new_spanned(
                    field.to_token_stream(),
                    "unexpected nameless field".to_owned(),
                )))
            };
                let attrs = match parse_mp4_attrs(&field.attrs) {
                    Ok(attrs) => attrs,
                    Err(e) => return Either::Right(Err(e)),
                };
                if attrs
                    .iter()
                    .any(|attr| matches!(attr, Mp4Attr::Name(name) if name.is_ident("header")))
                {
                    return Either::Left(field_name);
                }
                let field_name_str_lit = LitStr::new(&field_name.to_string(), field_name.span());
                let allocated_name =
                    Ident::new(&format!("field{}", idx), field.into_token_stream().span());
                Either::Right(Ok::<_, syn::Error>(StructFieldInfo {
                    ty: canonicalize_ty(&field.ty),
                    field_name: field_name.clone(),
                    field_name_str_lit,
                    allocated_name,
                }))
            });
        if headers.len() > 1 {
            return Err(syn::Error::new(
                span,
                "#[movparse(header)] attribute must be one".to_owned(),
            ));
        }
        let header = headers.into_iter().next();
        let fields = fields.into_iter().collect::<Result<Vec<_>, _>>()?;
        Ok(FieldsInfo::Struct {
            header_name: header.cloned(),
            fields,
        })
    } else {
        let fields = fields
            .iter()
            .enumerate()
            .map(|(idx, field)| {
                let allocated_name =
                    Ident::new(&format!("field{}", idx), field.into_token_stream().span());
                let attrs = match parse_mp4_attrs(&field.attrs) {
                    Ok(attrs) => attrs,
                    Err(e) => return Err(e),
                };
                if attrs
                    .iter()
                    .any(|attr| matches!(attr, Mp4Attr::Name(name) if name.is_ident("header")))
                {
                    return Ok(TupleFieldInfo::Header { allocated_name });
                }
                let allocated_name =
                    Ident::new(&format!("field{}", idx), field.into_token_stream().span());
                Ok::<_, syn::Error>(TupleFieldInfo::NormalField {
                    ty: canonicalize_ty(&field.ty),
                    allocated_name,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if fields
            .iter()
            .filter(|field| matches!(field, TupleFieldInfo::Header { .. }))
            .count()
            > 1
        {
            return Err(syn::Error::new(
                span,
                "#[movparse(header)] attribute must be one".to_owned(),
            ));
        }
        Ok(FieldsInfo::Tuple { fields })
    }
}

fn derive_mp4_root_read_for_struct(
    name: &Ident,
    strct: &syn::DataStruct,
) -> Result<TokenStream, syn::Error> {
    let fields_info = parse_fields(&strct.fields, strct.fields.span())?;
    let (placeholders, parsers, struct_return) = match fields_info {
        FieldsInfo::Struct {
            header_name,
            fields,
        } => {
            if let Some(header_name) = header_name {
                return Err(syn::Error::new(
                    header_name.span(),
                    "RootRead cannot has header".to_owned(),
                ));
            }
            let parsers = fields
                .iter()
                .map(|field| {
                    let allocated_name = &field.allocated_name;
                    quote! {
                        if #allocated_name.acceptable_tag(header.id) {
                            let value = #allocated_name.read_body(header, &mut reader2).await?;
                            ::movparse_box::BoxPlaceholder::push(&mut #allocated_name, value)?;
                            continue;
                        }
                    }
                })
                .fold(TokenStream2::new(), |mut acc, tokens| {
                    acc.append_all(tokens);
                    acc
                });
            let placeholders = fields
                .iter()
                .map(|field| {
                    let ty = &field.ty;
                    let allocated_name = &field.allocated_name;
                    quote! {
                        let mut #allocated_name = #ty::placeholder();
                    }
                })
                .fold(TokenStream2::new(), |mut acc, tokens| {
                    acc.append_all(tokens);
                    acc
                });
            let struct_fills = fields
                .iter()
                .map(|field| {
                    let actual_name = &field.field_name;
                    let allocated_name = &field.allocated_name;
                    let field_name_str = &field.field_name_str_lit;
                    quote! {
                        #actual_name: #allocated_name.get(#field_name_str)?,
                    }
                })
                .fold(TokenStream2::new(), |mut acc, tokens| {
                    acc.append_all(tokens);
                    acc
                });
            let struct_return = quote! {
                Ok(Self {
                    #struct_fills
                })
            };
            (placeholders, parsers, struct_return)
        }
        FieldsInfo::Tuple { fields } => {
            let internal_code_flakes = gen_code_flakes_for_internal_from_tuple(&fields)?;
            let struct_fills = fields
                .iter()
                .enumerate()
                .map(|(idx, field)| match field {
                    TupleFieldInfo::Header { allocated_name } => Err(syn::Error::new(
                        allocated_name.span(),
                        "#[movparse(header)] is unrecognized option for RootRead",
                    )),
                    TupleFieldInfo::NormalField {
                        ty: _,
                        allocated_name,
                    } => {
                        let field_name_str = syn::Lit::Str(syn::LitStr::new(
                            &format!("${}", idx),
                            allocated_name.span(),
                        ));
                        Ok(quote! {
                            #allocated_name.get(#field_name_str)?,
                        })
                    }
                })
                .fold(Ok::<_, syn::Error>(TokenStream2::new()), |acc, tokens| {
                    let mut acc = acc?;
                    let tokens = tokens?;
                    acc.append_all(tokens);
                    Ok(acc)
                })?;
            let struct_return = quote! {
                Ok(Self(
                    #struct_fills
                ))
            };
            (
                internal_code_flakes.placeholder_declations,
                internal_code_flakes.box_parsers,
                struct_return,
            )
        }
    };
    let derived = quote! {
        #[::async_trait::async_trait]
        impl ::movparse_box::RootRead for #name {
            async fn read<R: ::tokio::io::AsyncRead + ::tokio::io::AsyncSeek + ::std::marker::Unpin + ::std::marker::Send>(
                reader: &mut ::movparse_box::Reader<R>,
            ) -> std::io::Result<Self> {
                use ::movparse_box::{BoxContainer, BoxPlaceholder};
                let mut reader2 = reader.clone();
                #placeholders
                while reader2.remain() > 0 {
                    let header = ::movparse_box::BoxHeader::read(&mut reader2).await?;
                    #parsers
                    reader2.seek_from_current(header.body_size() as i64).await?;
                }
                #struct_return
            }
        }
    };
    Ok(derived.into())
}

fn gen_read_leaf_enum(name: &Ident, enm: &syn::DataEnum) -> Result<TokenStream, syn::Error> {
    let blocks = enm
        .variants
        .iter()
        .map(|variant| {
            let name = &variant.ident;
            let inner =
                gen_read_leaf_struct_inner(&quote! {Self::#name}, &variant.fields, variant.span())?;
            let acceptable_tag = acceptable_tag(&variant.attrs, &variant.span())?;
            Ok::<_, syn::Error>(quote! {
                if #acceptable_tag.contains(&header.id) {
                    #inner
                }
            })
        })
        .fold(Ok::<_, syn::Error>(TokenStream2::new()), |acc, block| {
            let mut acc = acc?;
            let block = block?;
            acc.append_all(block);
            Ok(acc)
        })?;

    let attrs_for_gen_acceptable_tag = enm
        .variants
        .iter()
        .flat_map(|variant| variant.attrs.clone())
        .collect_vec();

    let acceptable_tag = acceptable_tag(&attrs_for_gen_acceptable_tag, &enm.enum_token.span)?;

    let derived = quote! {
        #[::async_trait::async_trait]
        impl ::movparse_box::BoxRead for #name {
            fn acceptable_tag(tag: [u8;4]) -> bool {
                #acceptable_tag.contains(&tag)
            }

            async fn read_body<R: ::tokio::io::AsyncRead + ::tokio::io::AsyncSeek + ::std::marker::Unpin + ::std::marker::Send>(
                header: ::movparse_box::BoxHeader,
                reader: &mut ::movparse_box::Reader<R>,
            ) -> std::io::Result<Self> {
                #blocks
                unreachable!()
            }
        }
    };
    Ok(derived.into())
}

fn gen_read_internal_enum(name: &Ident, enm: &syn::DataEnum) -> Result<TokenStream, syn::Error> {
    let blocks = enm
        .variants
        .iter()
        .map(|variant| {
            let name = &variant.ident;
            let inner = gen_read_internal_struct_inner(
                &quote! {Self::#name},
                &variant.fields,
                variant.span(),
            )?;
            let acceptable_tag = acceptable_tag(&variant.attrs, &variant.span())?;
            Ok::<_, syn::Error>(quote! {
                if #acceptable_tag.contains(&header.id) {
                    #inner
                }
            })
        })
        .fold(Ok::<_, syn::Error>(TokenStream2::new()), |acc, block| {
            let mut acc = acc?;
            let block = block?;
            acc.append_all(block);
            Ok(acc)
        })?;

    let attrs_for_gen_acceptable_tag = enm
        .variants
        .iter()
        .flat_map(|variant| variant.attrs.clone())
        .collect_vec();

    let acceptable_tag = acceptable_tag(&attrs_for_gen_acceptable_tag, &enm.enum_token.span)?;

    let derived = quote! {
        fn acceptable_tag(tag: [u8;4]) -> bool {
            #acceptable_tag.contains(&tag)
        }

        #[::async_trait::async_trait]
        impl ::movparse_box::BoxRead for #name {
            async fn read_body<R: ::tokio::io::AsyncRead + ::tokio::io::AsyncSeek + ::std::marker::Unpin + ::std::marker::Send>(
                header: ::movparse_box::BoxHeader,
                reader: &mut ::movparse_box::Reader<R>,
            ) -> std::io::Result<Self> {
                #blocks
                unreachable!()
            }
        }
    };
    Ok(derived.into())
}

struct InternalCodeFlakes {
    box_parsers: TokenStream2,
    placeholder_declations: TokenStream2,
}

fn gen_code_flakes_for_internal_from_struct(
    fields: &[StructFieldInfo],
) -> syn::Result<InternalCodeFlakes> {
    let box_parsers = fields
        .iter()
        .map(|field| {
            let allocated_name = &field.allocated_name;
            quote! {
                if #allocated_name.acceptable_tag(header.id) {
                    let value = #allocated_name.read_body(header, &mut reader2).await?;
                    ::movparse_box::BoxPlaceholder::push(&mut #allocated_name, value)?;
                    continue;
                }
            }
        })
        .fold(TokenStream2::new(), |mut acc, tokens| {
            acc.append_all(tokens);
            acc
        });
    let placeholder_declations = fields
        .iter()
        .map(|field| {
            let ty = &field.ty;
            let allocated_name = &field.allocated_name;
            quote! {
                let mut #allocated_name = #ty::placeholder();
            }
        })
        .fold(TokenStream2::new(), |mut acc, tokens| {
            acc.append_all(tokens);
            acc
        });
    Ok(InternalCodeFlakes {
        placeholder_declations,
        box_parsers,
    })
}

fn gen_code_flakes_for_internal_from_tuple(
    fields: &[TupleFieldInfo],
) -> syn::Result<InternalCodeFlakes> {
    let box_parsers = fields
        .iter()
        .flat_map(|field| match field {
            TupleFieldInfo::NormalField {
                ty: _,
                allocated_name,
            } => Some({
                quote! {
                    if #allocated_name.acceptable_tag(header.id) {
                        let value = #allocated_name.read_body(header, &mut reader2).await?;
                        ::movparse_box::BoxPlaceholder::push(&mut #allocated_name, value)?;
                        continue;
                    }
                }
            }),
            TupleFieldInfo::Header { .. } => None,
        })
        .fold(TokenStream2::new(), |mut acc, tokens| {
            acc.append_all(tokens);
            acc
        });
    let placeholder_declations = fields
        .iter()
        .flat_map(|field| match field {
            TupleFieldInfo::NormalField { ty, allocated_name } => Some(quote! {
                let mut #allocated_name = #ty::placeholder();
            }),
            TupleFieldInfo::Header { .. } => None,
        })
        .fold(TokenStream2::new(), |mut acc, tokens| {
            acc.append_all(tokens);
            acc
        });
    Ok(InternalCodeFlakes {
        placeholder_declations,
        box_parsers,
    })
}

fn gen_read_internal_struct_inner(
    name: &TokenStream2,
    fields: &Fields,
    span: Span,
) -> Result<TokenStream2, syn::Error> {
    let fields_info = parse_fields(fields, fields.span())?;
    let (placeholders, parsers, struct_return) = match fields_info {
        FieldsInfo::Struct {
            header_name,
            fields,
        } => {
            let header_name = header_name.ok_or_else(|| {
                syn::Error::new(
                    span,
                    "boxtype = \"internal\" requires one #[movparse(header)]".to_owned(),
                )
            })?;
            let internal_code_flakes = gen_code_flakes_for_internal_from_struct(&fields)?;
            let struct_fills = fields
                .iter()
                .map(|field| {
                    let actual_name = &field.field_name;
                    let allocated_name = &field.allocated_name;
                    let field_name_str = &field.field_name_str_lit;
                    quote! {
                        #actual_name: #allocated_name.get(#field_name_str)?,
                    }
                })
                .fold(TokenStream2::new(), |mut acc, tokens| {
                    acc.append_all(tokens);
                    acc
                });
            let struct_return = quote! {
                Ok(#name {
                    #struct_fills
                    #header_name: header,
                })
            };
            (
                internal_code_flakes.placeholder_declations,
                internal_code_flakes.box_parsers,
                struct_return,
            )
        }
        FieldsInfo::Tuple { fields } => {
            let internal_code_flakes = gen_code_flakes_for_internal_from_tuple(&fields)?;
            let struct_fills = fields
                .iter()
                .enumerate()
                .map(|(idx, field)| match field {
                    TupleFieldInfo::Header { .. } => {
                        quote! {
                            header,
                        }
                    }
                    TupleFieldInfo::NormalField {
                        ty: _,
                        allocated_name,
                    } => {
                        let field_name_str = syn::Lit::Str(syn::LitStr::new(
                            &format!("${}", idx),
                            allocated_name.span(),
                        ));
                        quote! {
                            #allocated_name.get(#field_name_str)?,
                        }
                    }
                })
                .fold(TokenStream2::new(), |mut acc, tokens| {
                    acc.append_all(tokens);
                    acc
                });
            let struct_return = quote! {
                Ok(#name(
                    #struct_fills
                ))
            };
            (
                internal_code_flakes.placeholder_declations,
                internal_code_flakes.box_parsers,
                struct_return,
            )
        }
    };
    let derived = quote! {
        use ::movparse_box::{BoxContainer, BoxPlaceholder};
        let mut reader2 = reader.clone();
        reader2.set_limit(header.body_size() as u64);
        let mut next_seek = 0;
        #placeholders
        while reader2.remain() > 0 {
            let header = ::movparse_box::BoxHeader::read(&mut reader2).await?;
            #parsers
            reader2.seek_from_current(header.body_size() as i64).await?;
        }
        reader.seek_from_current(header.body_size() as i64).await?;
        return #struct_return
    };
    Ok(derived)
}

fn gen_read_internal_struct(
    name: &Ident,
    attrs: &Vec<Attribute>,
    strct: &syn::DataStruct,
) -> Result<TokenStream, syn::Error> {
    let inner = gen_read_internal_struct_inner(
        &name.to_token_stream(),
        &strct.fields,
        strct.struct_token.span,
    )?;
    let acceptable_tag = acceptable_tag(attrs, &strct.fields.span())?;

    let derived = quote! {
        #[::async_trait::async_trait]
        impl ::movparse_box::BoxRead for #name {

            fn acceptable_tag(tag: [u8;4]) -> bool {
                #acceptable_tag.contains(&tag)
            }

            async fn read_body<R: ::tokio::io::AsyncRead + ::tokio::io::AsyncSeek + ::std::marker::Unpin + ::std::marker::Send>(
                header: ::movparse_box::BoxHeader,
                reader: &mut ::movparse_box::Reader<R>,
            ) -> std::io::Result<Self> {
                #inner
            }
        }
    };
    Ok(derived.into())
}

enum Mp4BoxType {
    Leaf,
    Internal,
}

enum Mp4Attr {
    WithValue(syn::Path, syn::Lit),
    Name(syn::Path),
}

fn parse_mp4_attrs(attrs: &Vec<Attribute>) -> Result<Vec<Mp4Attr>, syn::Error> {
    let mut mp4_attrs = Vec::new();
    for attr in attrs {
        if attr.path.is_ident("mp4") {
            if let Ok(arg) = attr.parse_args::<ExprAssign>() {
                if !attr.path.is_ident("mp4") {
                    continue;
                }
                let (Expr::Path(left), Expr::Lit(lit)) = (arg.left.as_ref(), arg.right.as_ref()) else {
                    return Err(syn::Error::new_spanned(attr.tokens.clone(), "mp4(<attr> = <value>) required"));
                };
                mp4_attrs.push(Mp4Attr::WithValue(left.path.clone(), lit.lit.clone()));
            } else if let Ok(arg) = attr.parse_args::<ExprPath>() {
                mp4_attrs.push(Mp4Attr::Name(arg.path));
            } else {
                return Err(syn::Error::new_spanned(
                    attr.to_token_stream(),
                    "mp4() attribute accept only assign expr or single identity",
                ));
            }
        }
    }
    Ok(mp4_attrs)
}

fn read_mp4_box_type<T: ToTokens>(
    attrs: &Vec<Attribute>,
    tokens: T,
) -> Result<Mp4BoxType, syn::Error> {
    let attrs = parse_mp4_attrs(attrs)?;
    for attr in attrs {
        if let Mp4Attr::WithValue(tag, lit) = attr {
            if tag.is_ident("boxtype") {
                let syn::Lit::Str(s) = &lit else {
                    return Err(syn::Error::new_spanned(lit.to_token_stream(), "mp4(boxtype = <\"leaf\" | \"internal\">)"))
                };
                return match s.value().as_str() {
                    "leaf" => Ok(Mp4BoxType::Leaf),
                    "internal" => Ok(Mp4BoxType::Internal),
                    _ => Err(syn::Error::new_spanned(
                        lit.to_token_stream(),
                        "mp4(boxtype = <\"leaf\" | \"internal\">)",
                    )),
                };
            }
        }
    }

    Err(syn::Error::new_spanned(
        tokens,
        "mp4(boxtype = <string>) required",
    ))
}

#[proc_macro_derive(BoxRead, attributes(mp4))]
pub fn derive_mp4_box(input: TokenStream) -> TokenStream {
    let input = &parse_macro_input!(input as DeriveInput);
    let box_type = match read_mp4_box_type(&input.attrs, input.clone()) {
        Ok(v) => v,
        Err(e) => return e.to_compile_error().into(),
    };
    let result = match (&input.data, box_type) {
        (syn::Data::Struct(v), Mp4BoxType::Leaf) => {
            gen_read_leaf_struct(&input.ident, &input.attrs, v)
        }
        (syn::Data::Struct(v), Mp4BoxType::Internal) => {
            gen_read_internal_struct(&input.ident, &input.attrs, v)
        }
        (syn::Data::Enum(v), Mp4BoxType::Leaf) => gen_read_leaf_enum(&input.ident, v),
        (syn::Data::Enum(v), Mp4BoxType::Internal) => gen_read_internal_enum(&input.ident, v),
        (_, _) => panic!("union type is not supported"),
    };
    match result {
        Ok(generated) => generated,
        Err(e) => e.to_compile_error().into(),
    }
}

#[proc_macro_derive(RootRead, attributes(mp4))]
pub fn derive_root_read(input: TokenStream) -> TokenStream {
    let input = &parse_macro_input!(input as DeriveInput);

    let result = match &input.data {
        syn::Data::Struct(v) => derive_mp4_root_read_for_struct(&input.ident, v),
        _ => Err(syn::Error::new_spanned(&input.ident, "Must be struct Type")),
    };
    match result {
        Ok(generated) => generated,
        Err(e) => e.to_compile_error().into(),
    }
}
