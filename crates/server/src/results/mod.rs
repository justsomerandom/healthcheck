//! Result recording abstractions.

mod memory;
mod sink;

pub use memory::InMemoryResultSink;
pub use sink::{ResultSink, ResultSinkError};
