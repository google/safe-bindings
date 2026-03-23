use crate::make_option_type;

use flate2::GzHeader;
pub mod read;
pub mod write;

make_option_type!(GzHeader);
