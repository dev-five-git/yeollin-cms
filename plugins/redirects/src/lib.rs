//! Permanent, administrator-managed redirects for retired page URLs.

pub mod models;
pub mod routes;

yeollin_plugin::yeollin_plugin! {
    name: "redirects",
    author: "DevFive",
    description: "Permanent URL redirects before auth and static fallback",
    redirect_resolver: routes::resolve_redirect,
}

pub use models::rule;
