//! Free space on a drive.

use std::io;
use std::path::Path;

use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
use windows::core::HSTRING;

/// Bytes the current user may still write on the volume that holds the
/// directory `path` (`GetDiskFreeSpaceExW`, quotas included).
pub fn free_bytes(path: &Path) -> io::Result<u64> {
    // A UNC directory (`\\server\share\`) needs the trailing backslash.
    let mut dir = path.as_os_str().to_owned();
    if !dir.to_string_lossy().ends_with('\\') {
        dir.push("\\");
    }
    let mut free = 0u64;
    unsafe { GetDiskFreeSpaceExW(&HSTRING::from(dir), Some(&mut free), None, None) }?;
    Ok(free)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_dir_has_free_space() {
        let dir = tempfile::tempdir().unwrap();
        assert!(free_bytes(dir.path()).unwrap() > 0);
    }

    #[test]
    fn missing_directory_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(free_bytes(&dir.path().join("fehlt")).is_err());
    }
}
