mod nesting_depth;

mod quick_reader;
pub use quick_reader::*;

mod quick_writer;
pub use quick_writer::*;

#[path = "xmltree.rs"]
pub mod xmltree_bindings;
