#![cfg_attr(
    debug_assertions,
    allow(unused_imports, unused_variables, unused_mut, dead_code, unused_assignments)
)]

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DeriveInput, Fields, Ident, Type};

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
    let struct_name = &ast.ident;
    let config_loader_opts_ident = format_ident!("ConfigLoaderOpts");

    let fields = match &ast.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => fields,
            _ => unimplemented!("ConfigLoader only supports structs with named fields."),
        },
        _ => unimplemented!("ConfigLoader can only be derived for structs."),
    };

    let config_loader_opts_fields = fields.named.iter().map(|field| {
        let name = &field.ident;
        let ty = &field.ty;
        let option_ty = if is_option_type(ty) {
            quote! { #ty }
        } else {
            quote! { Option<#ty> }
        };

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

    let merge_function = {
        let field_merges = fields.named.iter().map(|field| {
            let name = &field.ident;
            quote! {
                #name: rhs.#name.clone().or_else(|| lhs.#name.clone()),
            }
        });

        quote! {
            pub fn merge(lhs: &Self, rhs: &Self) -> Self {
                Self {
                    #(#field_merges)*
                }
            }
        }
    };

    let resolve_function = {
        let field_resolutions = fields.named.iter().map(|field| {
            let name = &field.ident;
            quote! {
                #name: if cli_opts.#name.as_ref() != default_value_opts.#name.as_ref() {
                    cli_opts.#name.clone()
                } else {
                    precedence_opts.#name.clone()
                },
            }
        });

        quote! {
            pub fn resolve(cli_opts: &Self, default_value_opts: &Self, precedence_opts: &Self) -> Self {
                Self {
                    #(#field_resolutions)*
                }
            }
        }
    };

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

    let config_loader_opts_impl = quote! {
        #[derive(Clone, Debug, Default, serde::Deserialize, clap::Parser)]
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

    let from_impl_fields = fields.named.iter().map(|field| {
        let name = &field.ident;
        quote! {
            #name: config_opts.#name.take().unwrap_or_default()
        }
    });

    let from_impl = quote! {
        impl From<#config_loader_opts_ident> for #struct_name {
            fn from(mut config_opts: #config_loader_opts_ident) -> Self {
                Self {
                    #(#from_impl_fields,)*
                }
            }
        }
    };

    let load_config_impl = {
        let has_config_field = fields.named.iter().any(|field| {
            if let Some(ident) = &field.ident {
                if ident == "config" {
                    if let syn::Type::Path(type_path) = &field.ty {
                        return type_path.path.is_ident("String");
                    }
                }
            }
            false
        });
        if has_config_field {
            quote! {
                impl ConfigLoader for #struct_name {
                    fn load_config() -> Result<Self, Box<dyn std::error::Error>> {
                        let args: Vec<String> = std::env::args().collect();
                        let default_value_opts = #config_loader_opts_ident::parse_from([] as [&str; 0]);
                        let cli_opts = #config_loader_opts_ident::parse_from(args.as_slice());
                        let yml_opts = if let Some(config_path) = cli_opts.config.as_deref() {
                            if std::path::Path::new(config_path).exists() {
                                match std::fs::read_to_string(config_path) {
                                    Ok(config_contents) => serde_yaml::from_str(&config_contents)?,
                                    Err(_) => default_value_opts.clone(),
                                }
                            } else {
                                default_value_opts.clone()
                            }
                        } else {
                            default_value_opts.clone()
                        };
                        let precedence_opts = #config_loader_opts_ident::merge(&default_value_opts, &yml_opts);
                        let env_opts = #config_loader_opts_ident::from_env();
                        let precedence_opts = #config_loader_opts_ident::merge(&precedence_opts, &env_opts);
                        let final_opts = #config_loader_opts_ident::resolve(&cli_opts, &default_value_opts, &precedence_opts);
                        Ok(final_opts.into())
                    }
                }
            }
        } else {
            quote! {
                impl ConfigLoader for #struct_name {
                    fn load_config() -> Result<Self, Box<dyn std::error::Error>> {
                        let args: Vec<String> = std::env::args().collect();
                        let default_value_opts = #config_loader_opts_ident::parse_from([] as [&str; 0]);
                        let cli_opts = #config_loader_opts_ident::parse_from(args.as_slice());
                        let env_opts = #config_loader_opts_ident::from_env();
                        let precedence_opts = #config_loader_opts_ident::merge(&default_value_opts, &env_opts);
                        let final_opts = #config_loader_opts_ident::resolve(&cli_opts, &default_value_opts, &precedence_opts);
                        Ok(final_opts.into())
                    }
                }
            }
        }
    };

    quote! {
        #config_loader_opts_impl
        #from_impl
        #load_config_impl
    }
}
