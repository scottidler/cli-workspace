// src/lib.rs for `config-loader-macro` crate

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

// Define the procedural macro for `ConfigLoader`
#[proc_macro_derive(LoadConfig)]
pub fn load_config_derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let output = impl_config_loader(&ast);
    output.into()
}

fn impl_config_loader(ast: &DeriveInput) -> proc_macro2::TokenStream {
    let struct_name = &ast.ident; // Capture the struct's name.

    let fields = match &ast.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => fields,
            _ => unimplemented!("ConfigLoader only supports structs with named fields."),
        },
        _ => unimplemented!("ConfigLoader can only be derived for structs."),
    };

    let assignments = fields.named.iter().map(|field| {
        let name = field.ident.as_ref().expect("Named fields should have an identifier.");

        let name_str = name.to_string();

        quote! {
            #name: if let Ok(env_var) = std::env::var(#name_str.to_uppercase()) {
                env_var.parse().map_err(|_| format!("Failed to parse environment variable for {}", #name_str))?
            } else if let Some(config_value) = config.get(#name_str) {
                config_value.parse().map_err(|_| format!("Failed to parse config value for {}", #name_str))?
            } else {
                Default::default()
            }
        }
    });

    // Generate the default_values method
    let default_values_method = quote! {
        fn default_values() -> Result<Self, Box<dyn std::error::Error>> {
            Self::try_parse_from(&[""]).map_err(Into::into)
        }
    };

    // Generate the config_values method
    let config_values_method = quote! {
        fn config_values(config_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
            let cfg_contents = std::fs::read_to_string(config_path)?;
            let cfg: Self = serde_yaml::from_str(&cfg_contents)?;
            Ok(cfg)
        }
    };

    // Generate the load_config method
    let load_config_method = quote! {
        fn load_config() -> Result<Self, Box<dyn std::error::Error>> {
            let opts = #struct_name::parse(); // Assumes the struct implements `clap::Parser`

            let config_contents = std::fs::read_to_string(&opts.config)?;
            let config: std::collections::HashMap<String, String> = serde_yaml::from_str(&config_contents)?;

            Ok(Self {
                #(#assignments,)*
            })
        }
    };

    // Combine everything into the final impl block
    quote! {
        impl ConfigLoader for #struct_name {
            #default_values_method
            #config_values_method
            #load_config_method
        }
    }
}
