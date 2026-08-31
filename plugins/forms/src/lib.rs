//! Public forms with a privacy-conscious administrator submission inbox.

pub mod models;
pub mod routes;

yeollin_plugin::yeollin_plugin! {
    name: "forms",
    author: "DevFive",
    description: "Public forms and administrator submission inbox",
    public_api_routes: ["/public", "/submit"],
}

pub use models::{definition, submission};
