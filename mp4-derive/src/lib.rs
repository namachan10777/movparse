use proc_macro::TokenStream;
use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::{quote, ToTokens, TokenStreamExt};
use syn::{
    parse_macro_input,
    punctuated::Punctuated,
    token::{Bracket, Colon2, Comma, Semi},
    Attribute, DeriveInput, Expr, ExprAssign, ExprPath,
};

fn acceptable_tag(
    attrs: &Vec<Attribute>,
    strct: &syn::DataStruct,
) -> Result<TokenStream2, syn::Error> {
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
        return Err(syn::Error::new_spanned(
            strct.struct_token.to_token_stream(),
            "at least 1 mp4(tag = <4 char str>) required",
        ));
    }

    let tags_def = syn::Expr::Array(syn::ExprArray {
        attrs: Vec::new(),
        bracket_token: Bracket::default(),
        elems: tags,
    });

    Ok(quote! {
        fn acceptable_tag(tag: [u8;4]) -> bool {
            let tags = #tags_def;
            tags.contains(&tag)
        }
    })
}

fn derive_mp4_element_read_for_struct(
    name: &Ident,
    attrs: &Vec<Attribute>,
    strct: &syn::DataStruct,
) -> Result<TokenStream, syn::Error> {
    let mut assign_stmts = Punctuated::<_, Semi>::new();
    let mut field_names = Punctuated::<_, Comma>::new();
    let mut header_name = None;

    for field in &strct.fields {
        let attrs = parse_mp4_attrs(&field.attrs)?;
        if attrs.into_iter().any(|attr| {
            let Mp4Attr::Name(name) = attr else {return false};
            name.is_ident("header")
        }) {
            header_name = Some(&field.ident);
            continue;
        }
        let field_name = field.ident.as_ref().ok_or_else(|| {
            syn::Error::new_spanned(field.to_token_stream(), "each field requires field name")
        })?;
        assign_stmts.push(quote! {
            let #field_name = ::mp4_box::AttrRead::read(&mut reader2).await?;
        });
        field_names.push(field_name);
    }

    let header_name = header_name.ok_or_else(|| {
        syn::Error::new_spanned(
            strct.struct_token.to_token_stream(),
            "At least one #[mp4(header)] attribute required",
        )
    })?;

    let acceptable_tag = acceptable_tag(attrs, strct)?;

    let derived = quote! {
        #[::async_trait::async_trait]
        impl ::mp4_box::BoxRead for #name {

            #acceptable_tag

            async fn read<R: ::tokio::io::AsyncRead + ::tokio::io::AsyncSeek + ::std::marker::Unpin + ::std::marker::Send>(
                header: ::mp4_box::BoxHeader,
                reader: &mut ::mp4_box::Reader<R>,
            ) -> std::io::Result<Self> {
                let mut reader2 = reader.clone();
                reader2.set_limit(header.body_size() as u64);
                #assign_stmts
                reader.seek_from_current(header.body_size() as i64).await?;
                Ok(Self {
                    #header_name: header,
                    #field_names
                })
            }
        }
    };
    Ok(derived.into())
}

fn derive_mp4_root_read_for_struct(
    name: &Ident,
    strct: &syn::DataStruct,
) -> Result<TokenStream, syn::Error> {
    let mut parsers = TokenStream2::new();
    let mut placeholders = TokenStream2::new();
    let mut struct_fills = TokenStream2::new();

    for field in &strct.fields {
        let field_name = field.ident.as_ref().ok_or_else(|| {
            syn::Error::new_spanned(field.to_token_stream(), "each field requires field name")
        })?;
        let ty = &field.ty;
        let ty = match ty {
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
        };
        placeholders.append_all(quote! {
            let mut #field_name = #ty::placeholder();
        });
        struct_fills.append_all(quote! {
            #field_name: #field_name.get()?,
        });
        parsers.append_all(quote! {
            if #field_name.acceptable_tag(header.id) {
                let value = #field_name.read(header, &mut reader2).await?;
                ::mp4_box::BoxPlaceholder::push(&mut #field_name, value)?;
                continue;
            }
        });
    }

    let derived = quote! {
        #[::async_trait::async_trait]
        impl ::mp4_box::RootRead for #name {
            async fn read<R: ::tokio::io::AsyncRead + ::tokio::io::AsyncSeek + ::std::marker::Unpin + ::std::marker::Send>(
                reader: &mut ::mp4_box::Reader<R>,
            ) -> std::io::Result<Self> {
                use ::mp4_box::{BoxContainer, BoxPlaceholder};
                let mut reader2 = reader.clone();
                let mut next_seek = 0;
                #placeholders
                while reader2.remain() > 0 {
                    let header = ::mp4_box::BoxHeader::read(&mut reader2).await?;
                    #parsers
                    reader2.seek_from_current(header.body_size() as i64).await?;
                }
                Ok(Self {
                    #struct_fills
                })
            }
        }
    };
    Ok(derived.into())
}

fn derive_mp4_box_read_for_struct(
    name: &Ident,
    attrs: &Vec<Attribute>,
    strct: &syn::DataStruct,
) -> Result<TokenStream, syn::Error> {
    let mut parsers = TokenStream2::new();
    let mut placeholders = TokenStream2::new();
    let mut struct_fills = TokenStream2::new();
    let mut header_name = None;

    for field in &strct.fields {
        let attrs = parse_mp4_attrs(&field.attrs)?;
        if attrs.into_iter().any(|attr| {
            let Mp4Attr::Name(name) = attr else {return false};
            name.is_ident("header")
        }) {
            header_name = Some(&field.ident);
            continue;
        }
        let field_name = field.ident.as_ref().ok_or_else(|| {
            syn::Error::new_spanned(field.to_token_stream(), "each field requires field name")
        })?;
        let ty = &field.ty;
        let ty = match ty {
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
        };
        placeholders.append_all(quote! {
            let mut #field_name = #ty::placeholder();
        });
        struct_fills.append_all(quote! {
            #field_name: #field_name.get()?,
        });
        parsers.append_all(quote! {
            if #field_name.acceptable_tag(header.id) {
                let value = #field_name.read(header, &mut reader2).await?;
                ::mp4_box::BoxPlaceholder::push(&mut #field_name, value)?;
                continue;
            }
        });
    }

    let header_name = header_name.ok_or_else(|| {
        syn::Error::new_spanned(
            strct.struct_token.to_token_stream(),
            "At least one #[mp4(header)] attribute required",
        )
    })?;

    let acceptable_tag = acceptable_tag(attrs, strct)?;

    let derived = quote! {
        #[::async_trait::async_trait]
        impl ::mp4_box::BoxRead for #name {

            #acceptable_tag

            async fn read<R: ::tokio::io::AsyncRead + ::tokio::io::AsyncSeek + ::std::marker::Unpin + ::std::marker::Send>(
                header: ::mp4_box::BoxHeader,
                reader: &mut ::mp4_box::Reader<R>,
            ) -> std::io::Result<Self> {
                use ::mp4_box::{BoxContainer, BoxPlaceholder};
                let mut reader2 = reader.clone();
                reader2.set_limit(header.body_size() as u64);
                let mut next_seek = 0;
                #placeholders
                while reader2.remain() > 0 {
                    let header = ::mp4_box::BoxHeader::read(&mut reader2).await?;
                    #parsers
                    reader2.seek_from_current(header.body_size() as i64).await?;
                }
                reader.seek_from_current(header.body_size() as i64).await?;
                Ok(Self {
                    #header_name: header,
                    #struct_fills
                })
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
pub fn derive_mp4_box_leaf(input: TokenStream) -> TokenStream {
    let input = &parse_macro_input!(input as DeriveInput);

    let result = match &input.data {
        syn::Data::Struct(v) => match read_mp4_box_type(&input.attrs, input.clone()) {
            Ok(Mp4BoxType::Leaf) => {
                derive_mp4_element_read_for_struct(&input.ident, &input.attrs, v)
            }
            Ok(Mp4BoxType::Internal) => {
                derive_mp4_box_read_for_struct(&input.ident, &input.attrs, v)
            }
            Err(e) => return e.to_compile_error().into(),
        },
        _ => Err(syn::Error::new_spanned(&input.ident, "Must be struct Type")),
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
