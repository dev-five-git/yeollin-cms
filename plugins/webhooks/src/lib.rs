//! Signed event webhooks with SSRF-safe delivery.

mod delivery;
pub mod models;
mod routes;

use yeollin_plugin::SubscriberRegistration;

yeollin_plugin::yeollin_plugin! {
    name: "webhooks",
    author: "DevFive",
    description: "Signed event webhooks with SSRF-safe delivery",
    subscribers: [SubscriberRegistration::deferred(
        "deliver",
        [],
        delivery::deliver_event,
    )],
}
