use std::{fmt::Debug, path::PathBuf};

use anyhow::Context;

pub fn get_file_content<T: Into<PathBuf> + Debug + Clone>(t: T) -> String {
    let res = std::fs::read_to_string(&t.clone().into())
        .context(format!("Could not read file {:?}", t))
        .unwrap();
    res
}
