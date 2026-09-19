//! Shared library behind the `fsn` binary.
//!
//! `main.rs` is a thin CLI front-end; every module lives here so unit and
//! integration tests can link the model and the renderers directly.

pub mod attrpath;
pub mod cli;
pub mod error;
pub mod model;
pub mod nix;
pub mod pipeline;
pub mod render;
