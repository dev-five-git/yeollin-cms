//! Ranked full-text search backed by SQLite FTS5.

mod index;
mod routes;

yeollin_plugin::yeollin_plugin! {
    name: "search",
    author: "DevFive",
    description: "Ranked full-text content search backed by SQLite FTS5",
    on_init: index::initialize,
    subscribers: [index::content_subscriber()],
}
