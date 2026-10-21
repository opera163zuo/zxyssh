//! Plugin facade.
//!
//! The manifest specification — types, validation, command rendering, and the
//! built-in catalog — lives in the `zxyssh-plugins` workspace crate so it
//! can evolve independently of the desktop app and be reused by other
//! workspace members (CLI, future tooling). This module re-exports it to keep
//! existing `crate::plugins::*` call sites stable.

pub use zxyssh_plugins::*;
pub mod agent;
