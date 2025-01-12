use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::punctuated::Punctuated;

struct RPCFunction {
    ident: syn::Ident,
    inputs: Vec<syn::PatType>,
    output: Box<syn::Type>,
}

#[proc_macro_attribute]
pub fn serde_dispatch(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item_copy = item.clone();
    let item_trait = syn::parse_macro_input!(item_copy as syn::ItemTrait);
    let trait_ident = item_trait.ident;
    let server_trait_ident = format_ident!("{}RPCServer", trait_ident);
    let client_struct_ident = format_ident!("{}RPCClient", trait_ident);

    // collect errors
    let mut errors: Vec<syn::Error> = Vec::new();

    // extract signatures
    let signature: Vec<_> = item_trait
        .items
        .into_iter()
        .filter_map(|trait_item| {
            if let syn::TraitItem::Fn(trait_item_fn) = trait_item {
                Some(trait_item_fn.sig)
            } else {
                errors.push(syn::Error::new_spanned(
                    trait_item,
                    "the RPC trait should only have functions",
                ));
                None
            }
        })
        .collect();

    // extract relevant part of signatures
    let functions: Vec<RPCFunction> = signature
        .into_iter()
        .filter_map(|signature| {
            if let Some(token) = signature.constness {
                errors.push(syn::Error::new_spanned(
                    token,
                    "const functions are not supported for RPC",
                ));
                return None;
            }
            if let Some(token) = signature.asyncness {
                errors.push(syn::Error::new_spanned(
                    token,
                    "async functions are not supported for RPC",
                ));
                return None;
            }
            if signature.generics.lt_token.is_some() {
                errors.push(syn::Error::new_spanned(
                    signature.generics,
                    "generics are not supported for RPC",
                ));
                return None;
            }

            let mut inputs = signature.inputs.into_iter();
            let first_arg = inputs.next();
            if let Some(syn::FnArg::Receiver(_)) = first_arg {
            } else {
                errors.push(syn::Error::new_spanned(
                    first_arg,
                    "the RPC functions should have some form of `self` as first argument",
                ));
                return None;
            }
            let inputs = inputs
                .map(|fnarg| {
                    if let syn::FnArg::Typed(pat_type) = fnarg {
                        pat_type
                    } else {
                        // all functions arguments except the first one should be FnArg::Typed (not FnArg::Receiver)
                        unreachable!()
                    }
                })
                .collect();
            let output = match signature.output {
                syn::ReturnType::Default => {
                    let unit_type = syn::Type::Tuple(syn::TypeTuple {
                        paren_token: Default::default(),
                        elems: Punctuated::<syn::Type, syn::Token![,]>::new(),
                    });
                    Box::new(unit_type)
                }
                syn::ReturnType::Type(_, ty) => ty,
            };

            Some(RPCFunction {
                ident: signature.ident,
                inputs,
                output,
            })
        })
        .collect();

    let server_matches = functions
        .iter()
        .enumerate()
        .map(|(num, rpcfunction): (usize, _)| {
            let ident = &rpcfunction.ident;
            let inputs = &rpcfunction.inputs;
            let assignments = inputs.iter().map(|pattype| {
                quote! {
                    let #pattype = ::serde_dispatch::deserialize_from(&mut reader)?;
                }
            });
            let call_args = inputs.iter().map(|pattype| &pattype.pat);

            quote! {
                #num => {
                    #(#assignments)*
                    let result = self.#ident (#( #call_args ),*);
                    ::serde_dispatch::serialize_into(writer, &result)?;
                }
            }
        });
    let client_functions = functions
        .iter()
        .enumerate()
        .map(|(num, rpcfunction): (usize, _)| {
            let ident = &rpcfunction.ident;
            let inputs = &rpcfunction.inputs;
            let output = &rpcfunction.output;

            let args = inputs.iter().map(|pattype| {
                let pat = &pattype.pat;
                let ty = &pattype.ty;
                quote! { #pat : &#ty }
            });

            let serializations = inputs.iter().map(|pattype| {
                let pat = &pattype.pat;
                quote! { ::serde_dispatch::serialize_into(&mut self.writer, #pat)?; }
            });

            quote! {
                fn #ident (mut self, #(#args),*) -> ::std::result::Result<#output, ::serde_dispatch::Error> {
                    let function_id: ::std::primitive::usize = #num;
                    ::serde_dispatch::serialize_into(&mut self.writer, &function_id)?;

                    #(#serializations)*

                    let result = ::serde_dispatch::deserialize_from(self.reader)?;
                    std::result::Result::Ok(result)
                }
            }
        });

    let generated = quote! {
        trait #server_trait_ident : self::#trait_ident {
            fn handle_with<RServer, WServer>(
                &mut self,
                reader: RServer,
                writer: WServer,
            ) -> Result<(), ::serde_dispatch::Error>
            where
                RServer: ::std::io::Read,
                WServer: ::std::io::Write;
        }

        impl<T: self::#trait_ident> self::#server_trait_ident for T {
            fn handle_with<RServer, WServer>(
                &mut self,
                mut reader: RServer,
                writer: WServer,
            ) -> ::std::result::Result<(), ::serde_dispatch::Error>
            where
                RServer: ::std::io::Read,
                WServer: ::std::io::Write,
            {
                let function_id: ::std::primitive::usize = ::serde_dispatch::deserialize_from(&mut reader)?;
                match function_id {
                    #(#server_matches)*
                    _ => panic!("invalid function id")
                }
                ::std::result::Result::Ok(())
            }
        }

        struct #client_struct_ident <RClient, WClient> {
            reader: RClient,
            writer: WClient,
        }

        impl<RClient, WClient> self::#client_struct_ident <RClient, WClient>
        where
            RClient: ::std::io::Read,
            WClient: ::std::io::Write,
        {
            fn call_with(reader: RClient, writer: WClient) -> Self
            where
                RClient: ::std::io::Read,
                WClient: ::std::io::Write,
            {
                Self { reader, writer }
            }

            #(#client_functions)*
        }
    };

    let error_stream: proc_macro2::TokenStream =
        errors.iter().map(syn::Error::to_compile_error).collect();
    let error_stream: TokenStream = error_stream.into();
    let generated_stream: TokenStream = generated.into();
    let mut new_stream = TokenStream::new();
    new_stream.extend(item);
    new_stream.extend(error_stream);
    new_stream.extend(generated_stream);
    new_stream
}
