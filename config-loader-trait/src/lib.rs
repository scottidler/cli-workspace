// src/lib.rs for `config-loader-trait` crate

/// A trait for loading configuration into a struct.
pub trait ConfigLoader: Sized {
    fn default_values() -> Result<Self, Box<dyn std::error::Error>>;
    fn config_values(config_path: &str) -> Result<Self, Box<dyn std::error::Error>>;
    /// Load the configuration for the type implementing this trait.
    ///
    /// Returns the loaded configuration as an instance of the implementing type
    /// or an error if loading or parsing the configuration fails.
    fn load_config() -> Result<Self, Box<dyn std::error::Error>>;
}

// Depending on your setup, you might need to re-export items used by this trait.
// For example, if you use a specific error type in the trait methods that consumers of the
// trait will need to use, you should re-export it here.

// If the trait requires any external types from other crates in its interface, ensure to add those
// crates to the dependencies in Cargo.toml and re-export the necessary types so that anyone
// implementing the trait does not have to include those dependencies themselves.
