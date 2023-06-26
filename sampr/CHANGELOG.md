# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## 0.1.0

### Added

- Initial implementation of `Actor`, `Addr`, `Context`, and co.
- Types implementing `Message` can be sent to actors.
- Actors can implement `Handler<M>` to receive messages.
- Futures can be spawned in the actor's `Context`.
- Streams can be added to an actor.

