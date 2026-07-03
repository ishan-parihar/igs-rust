// Public API surface: `server` (IgsMcpServer), `config` (load_settings),
// `tools` (tool implementations + types). Everything else is pub(crate)
// to lock down the internal implementation from external consumers.
pub mod config;
pub mod server;
pub mod tools;

pub(crate) mod cache;
pub(crate) mod clustering;
pub(crate) mod fusion;
pub(crate) mod http;
pub(crate) mod obscura;
pub(crate) mod parsers;
pub(crate) mod persistence;
pub(crate) mod types;
