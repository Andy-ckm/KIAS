pub mod product;

/// Pre-1.0 source compatibility for callers using `routes::api::create_router`.
/// The implementation is the same Core-first product router.
pub mod api {
    pub use super::product::{create_router, create_router_with_surfaces};
}

pub use product::{create_router, create_router_with_surfaces};
