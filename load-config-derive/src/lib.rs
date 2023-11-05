#![cfg_attr(
    debug_assertions,
    allow(unused_imports, unused_variables, unused_mut, dead_code, unused_assignments)
)]

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DeriveInput, Fields, Ident, Type};

// Define the procedural macro for `ConfigLoader`
#[proc_macro_derive(LoadConfig)]
pub fn load_config_derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let output = impl_config_loader(&ast);
    output.into()
}

fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(last_segment) = type_path.path.segments.last() {
            return last_segment.ident == "Option";
        }
    }
    false
}

fn impl_config_loader(ast: &DeriveInput) -> proc_macro2::TokenStream {
    let struct_name = &ast.ident; // Capture the struct's name.
    let config_loader_opts_ident = format_ident!("ConfigLoaderOpts");

    let fields = match &ast.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => fields,
            _ => unimplemented!("ConfigLoader only supports structs with named fields."),
        },
        _ => unimplemented!("ConfigLoader can only be derived for structs."),
    };

    // Generate the ConfigLoaderOpts struct with Option types, avoiding Option<Option<T>>
    let config_loader_opts_fields = fields.named.iter().map(|field| {
        let name = &field.ident;
        let ty = &field.ty;
        let option_ty = if is_option_type(ty) {
            quote! { #ty }
        } else {
            quote! { Option<#ty> }
        };

        // Extract the clap attributes from the user's struct field
        let clap_attrs =
            field.attrs.iter().filter_map(
                |attr| {
                    if attr.path().is_ident("clap") {
                        Some(quote! { #attr })
                    } else {
                        None
                    }
                },
            );

        quote! {
            #(#clap_attrs)*
            pub #name: #option_ty,
        }
    });

    // Generate the merge function for ConfigLoaderOpts
    let merge_function = {
        let field_merges = fields.named.iter().map(|field| {
            let name = &field.ident;
            quote! {
                #name: rhs.#name.or(lhs.#name),
            }
        });

        quote! {
            pub fn merge(lhs: Self, rhs: Self) -> Self {
                Self {
                    #(#field_merges)*
                }
            }
        }
    };

    // Update the config_loader_opts_impl to include the merge function
    let config_loader_opts_impl = quote! {
        #[derive(Debug, serde::Deserialize, clap::Parser, Default)]
        #[serde(rename_all = "kebab-case")]
        struct #config_loader_opts_ident {
            #(#config_loader_opts_fields)*
        }

        impl #config_loader_opts_ident {
            #merge_function
        }
    };

    // Generate the From implementation for converting ConfigLoaderOpts to the user's struct
    let from_impl_fields = fields.named.iter().map(|field| {
        let name = &field.ident;
        // Use config_opts to access the fields instead of self
        quote! {
            #name: config_opts.#name.take().unwrap_or_default()
        }
    });

    // Generate the actual From implementation using the fields iterator
    let from_impl = quote! {
        impl From<#config_loader_opts_ident> for #struct_name {
            fn from(mut config_opts: #config_loader_opts_ident) -> Self {
                Self {
                    // Use the iterator to fill in the struct fields
                    #(#from_impl_fields,)*
                }
            }
        }
    };

    // Generate the load_config function
    let load_config_impl = quote! {
        impl ConfigLoader for #struct_name {
            fn load_config() -> Result<Self, Box<dyn std::error::Error>> {
                let args: Vec<String> = std::env::args().collect();
                eprintln!("args={:?}", args);

                let mut opts = #config_loader_opts_ident::parse_from(args.as_slice()); // removed ?
                eprintln!("1 opts={:?}", opts);

                // Load the YAML configuration file if specified
                let yml_opts = if let Some(ref config_path) = opts.config {
                    let config_contents = std::fs::read_to_string(config_path)?;
                    serde_yaml::from_str(&config_contents)?
                } else {
                    #config_loader_opts_ident::default()
                };
                eprintln!("yml_opts={:?}", yml_opts);

                opts = #config_loader_opts_ident::merge(opts, yml_opts);
                eprintln!("2 opts={:?}", opts);

                // Override with environment variables using envy
                let env_opts = envy::from_env::<#config_loader_opts_ident>().unwrap_or_default();
                eprintln!("env_opts={:?}", env_opts);

                opts = #config_loader_opts_ident::merge(opts, env_opts);
                eprintln!("3 opts={:?}", opts);

                // Reparse the CLI with all fields to allow for overrides
                let cli_opts = #config_loader_opts_ident::parse_from(args.as_slice()); // removed ?
                eprintln!("cli_opts={:?}", cli_opts);

                opts = #config_loader_opts_ident::merge(opts, cli_opts);
                eprintln!("4 opts={:?}", opts);

                // Convert to the user's struct
                Ok(opts.into())
            }
        }
    };

    // Combine the generated structs and impls into one TokenStream
    quote! {
        #config_loader_opts_impl
        #from_impl
        #load_config_impl
    }
}
