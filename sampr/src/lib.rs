//! **S**imple **A**sync **M**essage **P**assing, _sampr_, is a message passing framework based on
//! actors and inspired by [actix][1]
//!
//! The crate provides an abstraction to pass [Messages](Message) between [Actors](Actor). In
//! contrast to [actix][1] _sampr_ builds upon the rust language's async/await feature.
//!
//! # Overview
//!
//! An actor is an arbitrary type implementing the [Actor] trait. Each actor runs,
//! when started, in its own [Context], which is spawned in the [tokio](https:://tokio.rs) runtime
//! as [tokio::task](https://docs.rs/tokio/latest/tokio/task/index.html).
//!
//! ```
//! struct MyActor;
//!
//! impl sampr::Actor for MyActor{
//!   type Context = sampr::Context<Self>;
//! }
//! ```
//!
//! After an actor has started, it can receive messages from any other actor (or other components
//! of the program if the user whishes to do so). A messages is an arbitrary type implementing the
//! [Message] trait.
//!
//! ```
//! struct MyMessage(u8);
//!
//! impl sampr::Message for MyMessage {
//!   type Result = bool;
//! }
//! ```
//!
//! To receive a message, the actor type has to implement the [Handler] trait for this specific
//! message.
//!
//! ```
//! # struct MyMessage(u8);
//! # impl sampr::Message for MyMessage { type Result = bool; }
//! # struct MyActor;
//! # impl sampr::Actor for MyActor { type Context = sampr::Context<Self>; }
//! #[sampr::async_trait]
//! impl sampr::Handler<MyMessage> for MyActor {
//!   async fn handle(&mut self, msg: MyMessage, _ctx: &mut sampr::Context<Self>) -> bool {
//!     msg.0 == 0
//!   }
//! }
//! ```
//!
//! [1]: https://actix.rs

pub(crate) mod actor;
pub use actor::{Actor, Addr};

mod error;
pub use error::SamprError as Error;

pub(crate) mod context;
pub use context::{AsyncContext, Context};

pub(crate) mod message;
pub use message::{Handler, Message};

#[doc(no_inline)]
pub use async_trait::async_trait;
