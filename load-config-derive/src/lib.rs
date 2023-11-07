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

    // Generate the resolve function for ConfigLoaderOpts
    let resolve_function = {
        let field_resolutions = fields.named.iter().map(|field| {
            let name = &field.ident;
            quote! {
                #name: if cli_opts.#name != default_value_opts.#name {
                    cli_opts.#name
                } else {
                    precedence_opts.#name
                },
            }
        });

        quote! {
            pub fn resolve(cli_opts: Self, default_value_opts: Self, precedence_opts: Self) -> Self {
                Self {
                    #(#field_resolutions)*
                }
            }
        }
    };

    // Generate the from_env function for ConfigLoaderOpts
    let from_env_function = {
        let env_assignments = fields.named.iter().map(|field| {
            let ident = &field.ident;
            let ident_str = ident.as_ref().unwrap().to_string().to_uppercase();
            let ty = &field.ty;
            let option_wrapped = is_option_type(ty);

            let env_var_assignment = if option_wrapped {
                quote! {
                    std::env::var(#ident_str).ok()
                }
            } else {
                quote! {
                    std::env::var(#ident_str).ok().and_then(|s| s.parse().ok())
                }
            };

            quote! {
                #ident: #env_var_assignment
            }
        });

        quote! {
            pub fn from_env() -> Self {
                Self {
                    #(#env_assignments),*
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
            #resolve_function
            #from_env_function
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

                // get a struct with the default_value of every field by parsing with no args
                let default_value_opts = #config_loader_opts_ident::parse_from([] as [&str; 0]);

                let cli_opts = #config_loader_opts_ident::parse_from(args.as_slice());

                // Load the YAML configuration file if specified
                let yml_opts = if let Some(ref config_path) = cli_opts.config {
                    let config_contents = std::fs::read_to_string(config_path)?;
                    serde_yaml::from_str(&config_contents)?
                } else {
                    #config_loader_opts_ident::default()
                };

                let mut precedence_opts = #config_loader_opts_ident::merge(default_value_opts, yml_opts);

                let env_opts = #config_loader_opts_ident::from_env();

                precedence_opts = #config_loader_opts_ident::merge(precedence_opts, env_opts);

                // dumb as shit; get the default_value_opts again to avoid a move FIXME: terrible
                let default_value_opts = #config_loader_opts_ident::parse_from([] as [&str; 0]);

                // Override with CLI options if they differ from the default values
                let final_opts = #config_loader_opts_ident::resolve(cli_opts, default_value_opts, precedence_opts);

                // Convert to the user's struct
                Ok(final_opts.into())
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
