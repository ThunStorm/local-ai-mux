use std::path::PathBuf;

/// The only writable application data root. Never derive this from the working directory.
pub fn data_dir(application: &str) -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", application).map(|dirs| dirs.data_dir().to_path_buf())
}

#[cfg(test)]
mod tests {
    #[test]
    fn data_directory_is_absolute() {
        assert!(super::data_dir("LocalMux").is_some_and(|path| path.is_absolute()));
    }
}
