mod chmod;
mod chown;
mod contents;
mod copy;
mod create;
mod decrypt;
mod link;
mod remove;
mod unarchive;

pub use chmod::Chmod;
pub use chown::Chown;
pub use contents::SetContents;
pub use copy::Copy;
pub use create::Create;
pub use decrypt::Decrypt;
pub use link::Link;
pub use remove::Remove;
pub use unarchive::Unarchive;

use super::Atom;

pub trait FileAtom: Atom {
    // Don't think this is needed? Validate soon
    fn get_path(&self) -> &std::path::PathBuf;
}
