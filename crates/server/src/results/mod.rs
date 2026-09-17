//! Result recording abstractions.

mod memory;
mod sink;
mod sqlite;

pub use memory::InMemoryResultSink;
pub use sink::{ResultSink, ResultSinkError};
pub use sqlite::SqliteResultSink;
