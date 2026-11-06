//! Bounds that are `Send`/`Sync` on native targets and unbounded on `wasm32`,
//! where futures backed by JS promises can never be `Send`.

pub use target::{BoxStream, MaybeSend, MaybeSync};

#[cfg(not(target_arch = "wasm32"))]
mod target {
    use std::pin::Pin;

    use futures_core::Stream;

    pub trait MaybeSend: Send {}
    impl<T: Send + ?Sized> MaybeSend for T {}

    pub trait MaybeSync: Sync {}
    impl<T: Sync + ?Sized> MaybeSync for T {}

    pub type BoxStream<Item> = Pin<Box<dyn Stream<Item = Item> + Send>>;
}

#[cfg(target_arch = "wasm32")]
mod target {
    use std::pin::Pin;

    use futures_core::Stream;

    pub trait MaybeSend {}
    impl<T: ?Sized> MaybeSend for T {}

    pub trait MaybeSync {}
    impl<T: ?Sized> MaybeSync for T {}

    pub type BoxStream<Item> = Pin<Box<dyn Stream<Item = Item>>>;
}
