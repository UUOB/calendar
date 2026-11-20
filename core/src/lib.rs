#![recursion_limit = "512"]
//! Calendar without its screens, for Android and the Mac (UniFFI) and Linux
//! (directly). The rules are the web version's (app/lib), ported module by
//! module, so both versions read and write the same events; the answers of
//! the web version are kept as fixtures (tests/oracle) and checked here.

pub mod app;
pub mod details;
pub mod ffi;
pub mod ical;
pub mod instances;
pub mod recur;
pub mod series;
pub mod upcoming;
pub mod zones;

// The SDK is part of this library, so Kotlin and Swift get both.
pub use uuob_client;

uniffi::setup_scaffolding!();

#[uniffi::export]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").into()
}
