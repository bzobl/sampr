# Simple Async Message Passing in Rust

The _sampr_ crate provides a message passing framework inspired by
[actix](https://actix.rs). In contrast to the latter _sampr_ uses rust's
async/await language feature to deal with futures.

## Overview

_Actors_ are defined by implementing the `Actor` trait for an arbitrary
type.  An `Actor` can receive and process messages by implementing the
`Handler<M>` trait for that specific message. When started, each `Actor`
runs asynchronously as part of its `Context` in a separate
[`tokio::task`](https://docs.rs/tokio/latest/tokio/task/index.html).
Whenever a message is received the respective handler function is
called. As rust does, at the time of writing this, not support async
functions in traits, _sampr_ currently relies on
[async_trait](https://docs.rs/async-trait/latest/async_trait/).

## Current features

- Sending messages between actors and wait for their result.
- Spawning futures into an actor's context and waiting for it to resolve.
- Adding `Stream`s into an actor's context and waiting for it to produce items.
- Stopping an actor, moving the object back to the caller.

## Missing features

- Sending messages between actors without blocking the sender's
  `Context`, i.e., an `.send()` function taking a callback (this can be
  achieved already by spawning a task, but needs some syntactic sugar).
- Timers for doing work periodically.
