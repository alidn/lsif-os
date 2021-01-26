use std::{fmt::Debug, path::Path};

use anyhow::{Context, Result};

pub fn read_file<P: AsRef<Path> + Debug>(path: P) -> Result<String> {
    let res = std::fs::read_to_string(&path)
        .with_context(|| format!("Could not read file {:?}", path))?;
    Ok(res)
}
