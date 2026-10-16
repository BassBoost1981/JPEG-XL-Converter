//! What happens to the original after a verified conversion: the recycle-bin
//! rules, moving the file into the Windows recycle bin, and carrying the file
//! dates over to the new file.
//!
//! Nothing in here deletes a file. The only way a file leaves its place is
//! [`RecycleBin::recycle`], and the Windows implementation only ever asks the
//! shell to move it into the recycle bin.

use std::ffi::c_void;
use std::fs::{self, File, FileTimes};
use std::io;
use std::os::windows::fs::{FileTimesExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf, Prefix};
use std::sync::Mutex;
use std::thread;

use windows::Win32::Foundation::{
    E_ABORT, E_ACCESSDENIED, ERROR_CANCELLED, ERROR_FILE_NOT_FOUND, ERROR_LOCK_VIOLATION,
    ERROR_PATH_NOT_FOUND, ERROR_SHARING_VIOLATION, ERROR_SUCCESS,
};
use windows::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetDriveTypeW, GetVolumeNameForVolumeMountPointW, GetVolumePathNameW,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RegGetValueW,
};
use windows::Win32::UI::Shell::{
    COPYENGINE_E_ACCESS_DENIED_SRC, COPYENGINE_E_SHARING_VIOLATION_SRC,
    COPYENGINE_E_USER_CANCELLED, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT,
    FOF_WANTNUKEWARNING, FOFX_EARLYFAILURE, FileOperation, IFileOperation, IShellItem,
    SHCreateItemFromParsingName,
};
use windows::core::{HRESULT, HSTRING, PCWSTR, w};

const NOTE_NO_RECYCLE_BIN: &str = "Original behalten: kein Papierkorb auf diesem Laufwerk";
const NOTE_TOO_LARGE: &str = "Original behalten: zu groß für den Papierkorb";

/// Per-volume recycle-bin settings of the current user (HKCU).
const BITBUCKET_VOLUMES: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\BitBucket\Volume";
/// Explorer group policies, read below HKCU and HKLM.
const EXPLORER_POLICIES: &str = r"Software\Microsoft\Windows\CurrentVersion\Policies\Explorer";
/// `GetDriveTypeW` result for a fixed disk (WinBase.h).
const DRIVE_FIXED: u32 = 3;
/// Access right to change a file's attributes and times (WinNT.h).
const FILE_WRITE_ATTRIBUTES: u32 = 0x0100;

/// The facts that decide whether the original may go to the recycle bin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrashContext {
    /// Switch „Originale nach Prüfung in den Papierkorb“ is on.
    pub enabled: bool,
    /// The new file passed its check.
    pub verified: bool,
    /// The result is lossless.
    pub lossless: bool,
    /// All metadata was carried over.
    pub metadata_complete: bool,
}

/// True only when all four conditions hold.
pub fn trash_wanted(ctx: &TrashContext) -> bool {
    ctx.enabled && ctx.verified && ctx.lossless && ctx.metadata_complete
}

/// Whether a file can go into the recycle bin of its drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    Available,
    NoRecycleBin,
    TooLarge,
}

pub trait RecycleBin: Send + Sync {
    /// Checked before every move; `size` is the file size in bytes.
    fn availability(&self, path: &Path, size: u64) -> Availability;
    /// Moves the file into the recycle bin. `Err` carries a short German
    /// reason; the file is then expected to be where it was.
    fn recycle(&self, path: &Path) -> Result<(), String>;
}

/// The Windows recycle bin, through the shell (`IFileOperation`).
pub struct WindowsRecycleBin;

impl RecycleBin for WindowsRecycleBin {
    /// Fixed drive, volume known, and neither the Explorer policies nor the
    /// recycle-bin settings of that volume stand in the way. Whatever cannot
    /// be determined reliably counts as `NoRecycleBin`.
    fn availability(&self, path: &Path, size: u64) -> Availability {
        match bin_facts(path) {
            Some(facts) => decide(&facts, size),
            None => Availability::NoRecycleBin,
        }
    }

    fn recycle(&self, path: &Path) -> Result<(), String> {
        // Only a plain file is handed to the shell, never a folder or a link.
        match fs::symlink_metadata(path) {
            Ok(meta) if meta.is_file() => {}
            Ok(_) => return Err("Keine normale Datei".to_string()),
            Err(e) => return Err(io_reason(&e).to_string()),
        }
        let path = path.to_path_buf();
        // A fresh thread gets its own single-threaded COM apartment.
        thread::Builder::new()
            .spawn(move || recycle_in_sta(&path))
            .map_err(|_| "Interner Fehler".to_string())?
            .join()
            .unwrap_or_else(|_| Err("Interner Fehler".to_string()))
    }
}

/// Test double: answers with a fixed availability and records the paths it
/// was asked to recycle. It never touches a file.
pub struct FakeRecycleBin {
    pub availability: Availability,
    pub fail: bool,
    pub recycled: Mutex<Vec<PathBuf>>,
}

impl RecycleBin for FakeRecycleBin {
    fn availability(&self, _path: &Path, _size: u64) -> Availability {
        self.availability
    }

    fn recycle(&self, path: &Path) -> Result<(), String> {
        if self.fail {
            return Err("Testfehler".to_string());
        }
        self.recycled.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }
}

/// Moves `original` into the recycle bin when the rules allow it. Returns the
/// note for the queue entry when the original stays although the user wanted
/// it gone, otherwise `None`.
pub fn handle_original(
    ctx: &TrashContext,
    original: &Path,
    bin: &dyn RecycleBin,
) -> Option<String> {
    if !trash_wanted(ctx) {
        return None;
    }
    let meta = match fs::metadata(original) {
        Ok(meta) => meta,
        Err(e) => return Some(not_possible(io_reason(&e))),
    };
    match bin.availability(original, meta.len()) {
        Availability::Available => {}
        Availability::NoRecycleBin => return Some(NOTE_NO_RECYCLE_BIN.to_string()),
        Availability::TooLarge => return Some(NOTE_TOO_LARGE.to_string()),
    }
    // Explorer asks before it recycles a read-only file. Nobody is asked
    // here, so a write-protected original simply stays.
    if meta.permissions().readonly() {
        return Some(not_possible("Datei ist schreibgeschützt"));
    }
    bin.recycle(original)
        .err()
        .map(|reason| not_possible(&reason))
}

/// Gives `dst` the creation and modification time of `src`, exactly (100 ns).
pub fn copy_file_times(src: &Path, dst: &Path) -> io::Result<()> {
    let meta = fs::metadata(src)?;
    let times = FileTimes::new()
        .set_created(meta.created()?)
        .set_modified(meta.modified()?);
    // Opened for attribute changes only; the content stays untouched.
    File::options()
        .access_mode(FILE_WRITE_ATTRIBUTES)
        .open(dst)?
        .set_times(times)
}

fn not_possible(reason: &str) -> String {
    format!("Original behalten: Papierkorb nicht möglich ({reason})")
}

fn io_reason(e: &io::Error) -> &'static str {
    match e.kind() {
        io::ErrorKind::NotFound => "Datei nicht gefunden",
        _ => "Datei nicht lesbar",
    }
}

/// `X:\` when `path` lies on the volume of drive letter X: itself. `None`
/// for network paths, subst drives, junctions or links into another volume,
/// and volumes mounted in a folder: for those the recycle bin is not known
/// reliably.
fn local_drive_root(path: &Path) -> Option<String> {
    let letter = drive_letter(path)?;
    // Resolves subst drives, junctions and symbolic links.
    let real = fs::canonicalize(path).ok()?;
    if drive_letter(&real)? != letter {
        return None;
    }
    let mut mount = [0u16; 261];
    unsafe { GetVolumePathNameW(&HSTRING::from(real.as_path()), &mut mount) }.ok()?;
    let mount = wide_to_string(&mount);
    let root = format!(r"{}:\", char::from(letter));
    // A volume mounted in a folder answers with that folder, not `X:\`.
    let mount = mount.strip_prefix(r"\\?\").unwrap_or(&mount);
    mount.eq_ignore_ascii_case(&root).then_some(root)
}

/// Upper-case drive letter of `C:\…` and `\\?\C:\…`; `None` for anything else.
fn drive_letter(path: &Path) -> Option<u8> {
    match path.components().next()? {
        Component::Prefix(prefix) => match prefix.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
                Some(letter.to_ascii_uppercase())
            }
            _ => None,
        },
        _ => None,
    }
}

fn wide_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

/// Everything the recycle-bin decision for one volume depends on, as read
/// from Windows. `None` means "not set". Policy pairs are `[HKCU, HKLM]`.
#[derive(Debug, Clone, Copy)]
struct BinFacts {
    drive_type: u32,
    nuke_on_delete: Option<u32>,
    max_capacity_mb: Option<u32>,
    policy_no_recycle_files: [Option<u32>; 2],
    /// Percent of the volume.
    policy_recycle_bin_size: [Option<u32>; 2],
    volume_bytes: u64,
}

/// The recycle-bin rules.
/// - Not a fixed drive, `NoRecycleFiles` policy in either hive, or the
///   volume's `NukeOnDelete`: no recycle bin.
/// - `RecycleBinSize` policy (percent of the volume; the smaller one when
///   both hives set it) replaces the volume's own `MaxCapacity`; 0 % means
///   Windows deletes at once.
/// - Nothing set: Windows' defaults, which include a recycle bin.
fn decide(facts: &BinFacts, size: u64) -> Availability {
    let on = |value: Option<u32>| value.is_some_and(|v| v != 0);
    if facts.drive_type != DRIVE_FIXED
        || facts.policy_no_recycle_files.into_iter().any(on)
        || on(facts.nuke_on_delete)
    {
        return Availability::NoRecycleBin;
    }
    let capacity = match facts.policy_recycle_bin_size.into_iter().flatten().min() {
        Some(0) => return Availability::NoRecycleBin,
        Some(percent) => u128::from(facts.volume_bytes) * u128::from(percent.min(100)) / 100,
        None => match facts.max_capacity_mb {
            Some(mb) => u128::from(mb) * 1024 * 1024,
            None => return Availability::Available,
        },
    };
    if u128::from(size) <= capacity {
        Availability::Available
    } else {
        Availability::TooLarge
    }
}

/// Reads what `decide` needs for the volume that holds `path`. `None` when
/// any of it cannot be determined reliably.
fn bin_facts(path: &Path) -> Option<BinFacts> {
    let root = HSTRING::from(local_drive_root(path)?);
    let mut volume = [0u16; 64];
    unsafe { GetVolumeNameForVolumeMountPointW(&root, &mut volume) }.ok()?;
    // `\\?\Volume{GUID}\` -> `{GUID}`
    let volume = wide_to_string(&volume);
    let guid = volume.strip_prefix(r"\\?\Volume")?.strip_suffix('\\')?;
    let bitbucket = HSTRING::from(format!(r"{BITBUCKET_VOLUMES}\{guid}"));
    let policies = HSTRING::from(EXPLORER_POLICIES);
    let policy = |name| {
        Some([
            read_dword(HKEY_CURRENT_USER, &policies, name).ok()?,
            read_dword(HKEY_LOCAL_MACHINE, &policies, name).ok()?,
        ])
    };
    let mut volume_bytes = 0u64;
    unsafe { GetDiskFreeSpaceExW(&root, None, Some(&mut volume_bytes), None) }.ok()?;
    Some(BinFacts {
        drive_type: unsafe { GetDriveTypeW(&root) },
        nuke_on_delete: read_dword(HKEY_CURRENT_USER, &bitbucket, w!("NukeOnDelete")).ok()?,
        max_capacity_mb: read_dword(HKEY_CURRENT_USER, &bitbucket, w!("MaxCapacity")).ok()?,
        policy_no_recycle_files: policy(w!("NoRecycleFiles"))?,
        policy_recycle_bin_size: policy(w!("RecycleBinSize"))?,
        volume_bytes,
    })
}

/// A DWORD from the registry. `Ok(None)` when the key or the value does not
/// exist, `Err` when it exists but cannot be read as a DWORD.
fn read_dword(hive: HKEY, key: &HSTRING, value: PCWSTR) -> Result<Option<u32>, ()> {
    let mut data = 0u32;
    let mut len = size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            hive,
            key,
            value,
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut data).cast::<c_void>()),
            Some(&mut len),
        )
    };
    match status {
        ERROR_SUCCESS => Ok(Some(data)),
        ERROR_FILE_NOT_FOUND => Ok(None),
        _ => Err(()),
    }
}

/// Runs on its own thread: COM in a single-threaded apartment, the shell
/// operation, then the check that the file really left its place.
fn recycle_in_sta(path: &Path) -> Result<(), String> {
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
        .ok()
        .map_err(|e| reason(e.code()))?;
    // All COM objects live inside `shell_recycle` and are released before
    // CoUninitialize.
    let aborted = shell_recycle(path);
    unsafe { CoUninitialize() };
    match aborted {
        Err(e) => Err(reason(e.code())),
        Ok(true) => Err("Vorgang abgebrochen".to_string()),
        Ok(false) => match path.try_exists() {
            Ok(false) => Ok(()),
            Ok(true) => Err("Datei ist noch vorhanden".to_string()),
            Err(_) => Err("Ergebnis nicht prüfbar".to_string()),
        },
    }
}

/// `DeleteItem` with `FOF_ALLOWUNDO` moves the file into the recycle bin.
/// Should Windows want to delete it for good instead, `FOF_WANTNUKEWARNING`
/// makes it ask first, despite `FOF_NOCONFIRMATION`. Returns whether the
/// operation was aborted.
fn shell_recycle(path: &Path) -> windows::core::Result<bool> {
    unsafe {
        let op: IFileOperation = CoCreateInstance(&FileOperation, None, CLSCTX_ALL)?;
        op.SetOperationFlags(
            FOF_ALLOWUNDO
                | FOF_NOCONFIRMATION
                | FOF_SILENT
                | FOF_NOERRORUI
                | FOF_WANTNUKEWARNING
                | FOFX_EARLYFAILURE,
        )?;
        let item: IShellItem = SHCreateItemFromParsingName(&HSTRING::from(path), None)?;
        op.DeleteItem(&item, None)?;
        op.PerformOperations()?;
        Ok(op.GetAnyOperationsAborted()?.as_bool())
    }
}

/// Short German reason for a failed shell operation.
fn reason(code: HRESULT) -> String {
    const SHARING: HRESULT = HRESULT::from_win32(ERROR_SHARING_VIOLATION.0);
    const LOCK: HRESULT = HRESULT::from_win32(ERROR_LOCK_VIOLATION.0);
    const FILE_NOT_FOUND: HRESULT = HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0);
    const PATH_NOT_FOUND: HRESULT = HRESULT::from_win32(ERROR_PATH_NOT_FOUND.0);
    const CANCELLED: HRESULT = HRESULT::from_win32(ERROR_CANCELLED.0);
    let text = match code {
        E_ACCESSDENIED | COPYENGINE_E_ACCESS_DENIED_SRC => "Zugriff verweigert",
        SHARING | LOCK | COPYENGINE_E_SHARING_VIOLATION_SRC => {
            "Datei wird von einem anderen Programm verwendet"
        }
        FILE_NOT_FOUND | PATH_NOT_FOUND => "Datei nicht gefunden",
        E_ABORT | CANCELLED | COPYENGINE_E_USER_CANCELLED => "Vorgang abgebrochen",
        _ => return format!("Windows-Fehler 0x{:08X}", code.0 as u32),
    };
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn ctx(enabled: bool, verified: bool, lossless: bool, metadata_complete: bool) -> TrashContext {
        TrashContext {
            enabled,
            verified,
            lossless,
            metadata_complete,
        }
    }

    fn all_true() -> TrashContext {
        ctx(true, true, true, true)
    }

    fn fake(availability: Availability, fail: bool) -> FakeRecycleBin {
        FakeRecycleBin {
            availability,
            fail,
            recycled: Mutex::new(Vec::new()),
        }
    }

    fn original(dir: &tempfile::TempDir) -> PathBuf {
        let path = dir.path().join("foto.jpg");
        fs::write(&path, b"jpeg").unwrap();
        path
    }

    #[test]
    fn trash_needs_all_conditions() {
        for bits in 0..16u8 {
            let c = ctx(bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0);
            assert_eq!(trash_wanted(&c), bits == 0b1111, "{c:?}");
        }
    }

    #[test]
    fn unwanted_trash_leaves_original_alone() {
        let dir = tempfile::tempdir().unwrap();
        let file = original(&dir);
        let bin = fake(Availability::Available, false);
        for bits in 0..15u8 {
            let c = ctx(bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0);
            assert_eq!(handle_original(&c, &file, &bin), None, "{c:?}");
        }
        assert!(bin.recycled.lock().unwrap().is_empty());
        assert!(file.exists());
    }

    #[test]
    fn no_recycle_bin_keeps_original() {
        let dir = tempfile::tempdir().unwrap();
        let file = original(&dir);
        let bin = fake(Availability::NoRecycleBin, false);
        assert_eq!(
            handle_original(&all_true(), &file, &bin).as_deref(),
            Some("Original behalten: kein Papierkorb auf diesem Laufwerk")
        );
        assert!(bin.recycled.lock().unwrap().is_empty());
        assert!(file.exists());
    }

    #[test]
    fn too_large_keeps_original() {
        let dir = tempfile::tempdir().unwrap();
        let file = original(&dir);
        let bin = fake(Availability::TooLarge, false);
        assert_eq!(
            handle_original(&all_true(), &file, &bin).as_deref(),
            Some("Original behalten: zu groß für den Papierkorb")
        );
        assert!(bin.recycled.lock().unwrap().is_empty());
        assert!(file.exists());
    }

    #[test]
    fn recycle_failure_keeps_original() {
        let dir = tempfile::tempdir().unwrap();
        let file = original(&dir);
        let bin = fake(Availability::Available, true);
        let note = handle_original(&all_true(), &file, &bin).unwrap();
        assert_eq!(
            note,
            "Original behalten: Papierkorb nicht möglich (Testfehler)"
        );
        assert!(file.exists());
    }

    #[test]
    fn read_only_original_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let file = original(&dir);
        let mut perms = fs::metadata(&file).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&file, perms.clone()).unwrap();

        let bin = fake(Availability::Available, false);
        let note = handle_original(&all_true(), &file, &bin);

        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        fs::set_permissions(&file, perms).unwrap();
        assert_eq!(
            note.as_deref(),
            Some("Original behalten: Papierkorb nicht möglich (Datei ist schreibgeschützt)")
        );
        assert!(bin.recycled.lock().unwrap().is_empty());
    }

    #[test]
    fn missing_original_is_not_recycled() {
        let dir = tempfile::tempdir().unwrap();
        let bin = fake(Availability::Available, false);
        let note = handle_original(&all_true(), &dir.path().join("weg.jpg"), &bin);
        assert_eq!(
            note.as_deref(),
            Some("Original behalten: Papierkorb nicht möglich (Datei nicht gefunden)")
        );
        assert!(bin.recycled.lock().unwrap().is_empty());
    }

    #[test]
    fn recycles_when_allowed() {
        let dir = tempfile::tempdir().unwrap();
        let file = original(&dir);
        let bin = fake(Availability::Available, false);
        assert_eq!(handle_original(&all_true(), &file, &bin), None);
        assert_eq!(*bin.recycled.lock().unwrap(), vec![file]);
    }

    #[test]
    fn copy_file_times_copies_created_and_modified() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.jpg");
        let dst = dir.path().join("a.jxl");
        fs::write(&src, b"a").unwrap();
        fs::write(&dst, b"b").unwrap();
        // 2001-02-03 04:05:06 and 05:06:07 UTC.
        let created = SystemTime::UNIX_EPOCH + Duration::from_secs(981_173_106);
        let modified = SystemTime::UNIX_EPOCH + Duration::from_secs(981_176_767);
        File::options()
            .write(true)
            .open(&src)
            .unwrap()
            .set_times(FileTimes::new().set_created(created).set_modified(modified))
            .unwrap();

        copy_file_times(&src, &dst).unwrap();

        let meta = fs::metadata(&dst).unwrap();
        assert_eq!(meta.created().unwrap(), created);
        assert_eq!(meta.modified().unwrap(), modified);
        assert_eq!(fs::read(&dst).unwrap(), b"b");
    }

    #[test]
    fn copy_file_times_fails_for_missing_source() {
        let dir = tempfile::tempdir().unwrap();
        let dst = dir.path().join("a.jxl");
        fs::write(&dst, b"b").unwrap();
        assert!(copy_file_times(&dir.path().join("weg.jpg"), &dst).is_err());
    }

    #[test]
    fn failure_reasons_are_short_german_texts() {
        use windows::Win32::Foundation::{E_ACCESSDENIED, ERROR_SHARING_VIOLATION};
        use windows::Win32::UI::Shell::COPYENGINE_E_USER_CANCELLED;
        assert_eq!(reason(E_ACCESSDENIED), "Zugriff verweigert");
        assert_eq!(
            reason(HRESULT::from_win32(ERROR_SHARING_VIOLATION.0)),
            "Datei wird von einem anderen Programm verwendet"
        );
        assert_eq!(reason(COPYENGINE_E_USER_CANCELLED), "Vorgang abgebrochen");
        assert_eq!(
            reason(HRESULT(0x8027_0058_u32 as i32)),
            "Windows-Fehler 0x80270058"
        );
    }

    const GB: u64 = 1_000_000_000;
    const MIB: u64 = 1024 * 1024;

    /// A fixed drive of 100 GB with nothing configured.
    fn facts() -> BinFacts {
        BinFacts {
            drive_type: DRIVE_FIXED,
            nuke_on_delete: None,
            max_capacity_mb: None,
            policy_no_recycle_files: [None, None],
            policy_recycle_bin_size: [None, None],
            volume_bytes: 100 * GB,
        }
    }

    #[test]
    fn bin_rules_without_policy() {
        assert_eq!(decide(&facts(), 50 * GB), Availability::Available);
        let removable = BinFacts {
            drive_type: 2,
            ..facts()
        };
        assert_eq!(decide(&removable, 1), Availability::NoRecycleBin);
        for (nuke, expected) in [
            (Some(0), Availability::Available),
            (Some(1), Availability::NoRecycleBin),
            (Some(7), Availability::NoRecycleBin),
        ] {
            let f = BinFacts {
                nuke_on_delete: nuke,
                ..facts()
            };
            assert_eq!(decide(&f, 1), expected, "{nuke:?}");
        }
        let capped = BinFacts {
            nuke_on_delete: Some(0),
            max_capacity_mb: Some(100),
            ..facts()
        };
        assert_eq!(decide(&capped, 100 * MIB), Availability::Available);
        assert_eq!(decide(&capped, 100 * MIB + 1), Availability::TooLarge);
    }

    #[test]
    fn policy_no_recycle_files_means_no_recycle_bin() {
        // The volume's own settings would allow it.
        let bin = BinFacts {
            nuke_on_delete: Some(0),
            max_capacity_mb: Some(50_000),
            ..facts()
        };
        for policy in [[Some(1), None], [None, Some(1)], [Some(0), Some(1)]] {
            let f = BinFacts {
                policy_no_recycle_files: policy,
                ..bin
            };
            assert_eq!(decide(&f, 1), Availability::NoRecycleBin, "{policy:?}");
        }
        let off = BinFacts {
            policy_no_recycle_files: [Some(0), Some(0)],
            ..bin
        };
        assert_eq!(decide(&off, 1), Availability::Available);
    }

    #[test]
    fn policy_recycle_bin_size_sets_the_capacity() {
        let ten_percent = |hkcu, hklm, max_capacity_mb| BinFacts {
            nuke_on_delete: Some(0),
            max_capacity_mb,
            policy_recycle_bin_size: [hkcu, hklm],
            ..facts()
        };
        // 10 % of 100 GB, from either hive.
        for f in [
            ten_percent(Some(10), None, None),
            ten_percent(None, Some(10), None),
        ] {
            assert_eq!(decide(&f, 15 * GB), Availability::TooLarge);
            assert_eq!(decide(&f, 10 * GB), Availability::Available);
        }
        // The policy overrides MaxCapacity in both directions.
        let big_volume_setting = ten_percent(Some(10), None, Some(50_000));
        assert_eq!(decide(&big_volume_setting, 15 * GB), Availability::TooLarge);
        let small_volume_setting = ten_percent(Some(10), None, Some(1));
        assert_eq!(
            decide(&small_volume_setting, 5 * GB),
            Availability::Available
        );
        // Set in both hives: the smaller one counts.
        let both = ten_percent(Some(50), Some(10), None);
        assert_eq!(decide(&both, 15 * GB), Availability::TooLarge);
        // More than 100 % is read as 100 %.
        let over = ten_percent(Some(250), None, None);
        assert_eq!(decide(&over, 100 * GB), Availability::Available);
        assert_eq!(decide(&over, 100 * GB + 1), Availability::TooLarge);
    }

    #[test]
    fn policy_recycle_bin_size_zero_means_no_recycle_bin() {
        for policy in [[Some(0), None], [None, Some(0)], [Some(10), Some(0)]] {
            let f = BinFacts {
                policy_recycle_bin_size: policy,
                ..facts()
            };
            assert_eq!(decide(&f, 1), Availability::NoRecycleBin, "{policy:?}");
        }
    }

    #[test]
    fn network_paths_have_no_recycle_bin() {
        for path in [r"\\server\share\foto.jpg", r"\\?\UNC\server\share\foto.jpg"] {
            assert_eq!(
                WindowsRecycleBin.availability(Path::new(path), 1),
                Availability::NoRecycleBin,
                "{path}"
            );
        }
    }

    #[test]
    fn windows_recycle_refuses_what_is_not_a_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(WindowsRecycleBin.recycle(dir.path()).is_err());
        assert!(dir.path().is_dir());
        assert!(
            WindowsRecycleBin
                .recycle(&dir.path().join("weg.jpg"))
                .is_err()
        );
    }

    /// Moves a small file it creates in %TEMP% into the real recycle bin.
    /// Run by hand only: `cargo test -- --ignored windows_recycle_bin_manual`.
    #[test]
    #[ignore]
    fn windows_recycle_bin_manual() {
        let path = std::env::temp_dir().join(format!(
            "JPEG XL Konverter Papierkorbtest {}.txt",
            std::process::id()
        ));
        fs::write(
            &path,
            b"Testdatei des JPEG XL Konverters, darf geloescht werden.",
        )
        .unwrap();
        assert_eq!(
            WindowsRecycleBin.availability(&path, 64),
            Availability::Available
        );
        WindowsRecycleBin.recycle(&path).unwrap();
        assert!(!path.exists());
        println!("Im Papierkorb: {}", path.display());
    }
}
