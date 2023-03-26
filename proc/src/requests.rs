use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::ToTokens;
use syn::{
    braced,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token::{Paren, RArrow},
    Attribute, Field, FieldsUnnamed, Result, Token, Type,
};

pub fn requests(item: TokenStream) -> TokenStream {
    let defs = syn::parse_macro_input!(item as Definitions);
    let arms = defs.arms;

    let mut reqs = Vec::new();

    for def in &arms {
        reqs.push({
            let attrs = &def.attrs;
            let ident = &def.ident;
            let req = &def.req;
            let response = &def.resp;
            let name = ident.to_string();
            let path = {
                let mut s = DefaultHasher::new();
                def.req.to_token_stream().to_string().hash(&mut s);
                let req_hash = s.finish();
                let mut s = DefaultHasher::new();
                def.resp.to_token_stream().to_string().hash(&mut s);
                let resp_hash = s.finish();
                format!("{name}-{req_hash}-{resp_hash}")
            };
            quote::quote!(
                #[derive(Debug, serde::Serialize, serde::Deserialize)]
                #(#attrs)*
                struct #ident #req
                #[mees::async_trait::async_trait]
                impl mees::Requestable for #ident {
                    type Response = #response;
                    fn path() -> &'static str {
                        #path
                    }
                }
            )
        });
    }
    TokenStream::from(quote::quote!(
        #(#reqs)*
    ))
}

struct Definitions {
    arms: Vec<Definition>,
}

impl Parse for Definitions {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut arms = Vec::new();
        while !input.is_empty() {
            arms.push(input.parse::<Definition>()?);
        }
        Ok(Self { arms })
    }
}

struct Definition {
    attrs: Vec<Attribute>,
    ident: Ident,
    req: Box<Data>,
    _fat_arrow_token: RArrow,
    resp: Box<Type>,
}

impl Parse for Definition {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            attrs: input.call(Attribute::parse_outer)?,
            ident: input.parse::<Ident>()?,
            req: Box::new(input.parse::<Data>()?),
            _fat_arrow_token: input.parse::<RArrow>()?,
            resp: Box::new(input.parse::<Type>()?),
        })
    }
}

pub enum Data {
    Named(Punctuated<Field, Token![,]>),
    Unnamed(FieldsUnnamed),
}

impl Parse for Data {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek(Paren) {
            Ok(Self::Unnamed(input.parse::<FieldsUnnamed>()?))
        } else {
            let content;
            braced!(content in input);
            Ok(Self::Named(
                content.parse_terminated(Field::parse_named, Token![,])?,
            ))
        }
    }
}

impl ToTokens for Data {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Self::Named(fields) => {
                tokens.extend(quote::quote!({
                    #fields
                }));
            }
            Self::Unnamed(ty) => {
                tokens.extend(quote::quote!(#ty;));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_response_type_named() {
        let input = quote::quote!({
            foo: String,
            bar: i32,
        });
        let output = syn::parse2::<Data>(input).unwrap();
        assert_eq!(
            output.to_token_stream().to_string(),
            "{ foo : String , bar : i32 , }"
        );
    }

    #[test]
    fn parse_response_type_unnamed() {
        let input = quote::quote!((String, i32));
        let output = syn::parse2::<Data>(input).unwrap();
        assert_eq!(output.to_token_stream().to_string(), "(String , i32) ;");
    }
}
