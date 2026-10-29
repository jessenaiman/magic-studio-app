//! Read-only opens must not block on FIFOs or follow a replaced final symlink.
use crate::{EngineError, Result};
use std::{
    fs::{File, OpenOptions},
    path::Path,
};

pub(crate) fn open_regular(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(
            (rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::NOFOLLOW).bits() as i32,
        );
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(EngineError::InvalidParams("Expected a regular file".into()));
    }
    Ok(file)
}
