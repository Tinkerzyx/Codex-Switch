use std::{
    fs::{self, File, OpenOptions},
    io,
    path::Path,
};

pub(crate) const SCHEMA_VERSION: u32 = 1;
pub(crate) fn schema_version() -> u32 {
    SCHEMA_VERSION
}

pub(crate) fn private_dir(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

pub(crate) fn private_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    Ok(file)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn credentials_have_owner_only_permissions() {
        let dir = std::env::temp_dir().join(format!("csw-permissions-{}", uuid::Uuid::new_v4()));
        private_dir(&dir).unwrap();
        let file = dir.join("auth.json");
        drop(private_file(&file).unwrap());
        assert_eq!(
            fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::remove_file(file).unwrap();
        fs::remove_dir(dir).unwrap();
    }
}
