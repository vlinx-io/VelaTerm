//! System-level file clipboard interoperability for the desktop Files panel.
//!
//! Windows writes and reads `CF_HDROP` paired with `"Preferred DropEffect"` so cut and copy
//! operations interoperate directly with File Explorer and other desktop applications. macOS reads
//! and writes file URLs on `NSPasteboard.generalPasteboard` while tracking cut intent by pasteboard
//! `changeCount`. Linux currently reports `supported: false` so the frontend keeps using its
//! in-app file clipboard until native X11/Wayland file-URI support is added.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileClipboardMode {
    Copy,
    Cut,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileClipboardContent {
    pub paths: Vec<String>,
    pub mode: FileClipboardMode,
    pub sequence: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileClipboardStatus {
    pub supported: bool,
    pub has_files: bool,
    pub mode: Option<FileClipboardMode>,
    pub sequence: u64,
}

/// Tracks the last cut/copy written by this process along with the clipboard sequence number at the
/// time of the write, so macOS can distinguish cut from copy and all platforms notice when another
/// application overwrites the clipboard.
static LAST_WRITE: Mutex<Option<(u64, FileClipboardMode)>> = Mutex::new(None);

fn record_write(sequence: u64, mode: FileClipboardMode) {
    if let Ok(mut guard) = LAST_WRITE.lock() {
        *guard = Some((sequence, mode));
    }
}

fn recorded_mode_for(sequence: u64) -> Option<FileClipboardMode> {
    LAST_WRITE
        .lock()
        .ok()
        .and_then(|guard| guard.filter(|(seq, _)| *seq == sequence).map(|(_, mode)| mode))
}

fn clear_recorded_write() {
    if let Ok(mut guard) = LAST_WRITE.lock() {
        *guard = None;
    }
}

pub fn status() -> FileClipboardStatus {
    platform::status()
}

pub fn write_files(paths: &[String], mode: FileClipboardMode) -> Result<u64, String> {
    if paths.is_empty() {
        return Err("No files selected".to_string());
    }
    let sequence = platform::write_files(paths, mode)?;
    record_write(sequence, mode);
    Ok(sequence)
}

pub fn read_files() -> Result<Option<FileClipboardContent>, String> {
    platform::read_files()
}

pub fn clear_if_sequence(expected_sequence: Option<u64>) -> Result<(), String> {
    platform::clear_if_sequence(expected_sequence)?;
    clear_recorded_write();
    Ok(())
}

/// Normalize a filesystem path for Windows `CF_HDROP` (`\` separators, no `\\?\` verbatim prefix).
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn to_windows_clipboard_path(raw: &str) -> String {
    let stripped = if let Some(rest) = raw.strip_prefix("\\\\?\\UNC\\") {
        format!("\\\\{rest}")
    } else if let Some(rest) = raw.strip_prefix("\\\\?\\") {
        rest.to_string()
    } else {
        raw.to_string()
    };
    let mut normalized = String::with_capacity(stripped.len());
    let is_unc = stripped.starts_with("\\\\") || stripped.starts_with("//");
    let rest = if is_unc {
        normalized.push_str("\\\\");
        &stripped[2..]
    } else {
        stripped.as_str()
    };
    let mut prev_sep = false;
    for ch in rest.chars() {
        if ch == '/' || ch == '\\' {
            if !prev_sep {
                normalized.push('\\');
                prev_sep = true;
            }
        } else {
            normalized.push(ch);
            prev_sep = false;
        }
    }
    // Preserve drive roots such as `C:\`, but trim trailing backslashes from ordinary paths.
    let bytes = normalized.as_bytes();
    if normalized.len() > 3 && bytes.last() == Some(&b'\\') {
        normalized.pop();
    } else if is_unc && normalized.len() > 2 && bytes.last() == Some(&b'\\') {
        normalized.pop();
    }
    normalized
}

/// Pack file paths into a Win32 `CF_HDROP` buffer (`DROPFILES` header + UTF-16LE paths + double NUL).
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn encode_dropfiles_wide(paths: &[String]) -> Vec<u8> {
    const DROPFILES_SIZE: u32 = 20;
    let mut buf = Vec::with_capacity(DROPFILES_SIZE as usize + paths.len() * 64);
    buf.extend_from_slice(&DROPFILES_SIZE.to_le_bytes()); // pFiles
    buf.extend_from_slice(&0i32.to_le_bytes()); // pt.x
    buf.extend_from_slice(&0i32.to_le_bytes()); // pt.y
    buf.extend_from_slice(&0i32.to_le_bytes()); // fNC
    buf.extend_from_slice(&1i32.to_le_bytes()); // fWide = TRUE
    for path in paths {
        for unit in path.encode_utf16() {
            buf.extend_from_slice(&unit.to_le_bytes());
        }
        buf.extend_from_slice(&0u16.to_le_bytes());
    }
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf
}

/// Parse a Win32 `CF_HDROP` buffer when `fWide != 0`. Returns `None` if malformed or ANSI (`fWide == 0`).
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn decode_dropfiles_wide(bytes: &[u8]) -> Option<Vec<String>> {
    const DROPFILES_SIZE: usize = 20;
    if bytes.len() < DROPFILES_SIZE {
        return None;
    }
    let p_files = u32::from_le_bytes(bytes[0..4].try_into().ok()?) as usize;
    let f_wide = u32::from_le_bytes(bytes[16..20].try_into().ok()?);
    if f_wide == 0 || p_files < DROPFILES_SIZE || p_files > bytes.len() {
        return None;
    }
    let mut paths = Vec::new();
    let mut current: Vec<u16> = Vec::new();
    for pair in bytes[p_files..].chunks_exact(2) {
        let unit = u16::from_le_bytes([pair[0], pair[1]]);
        if unit == 0 {
            if current.is_empty() {
                break;
            }
            paths.push(String::from_utf16_lossy(&current));
            current.clear();
        } else {
            current.push(unit);
        }
    }
    if !current.is_empty() {
        paths.push(String::from_utf16_lossy(&current));
    }
    Some(paths)
}

#[cfg(windows)]
mod platform {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::time::Duration;

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
    use windows::Win32::Globalization::{MultiByteToWideChar, CP_ACP, MULTI_BYTE_TO_WIDE_CHAR_FLAGS};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
        IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
    };
    use windows::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
    };

    use super::{
        decode_dropfiles_wide, encode_dropfiles_wide, recorded_mode_for, to_windows_clipboard_path,
        FileClipboardContent, FileClipboardMode, FileClipboardStatus,
    };

    const CF_HDROP: u32 = 15;
    const DROPEFFECT_COPY: u32 = 1;
    const DROPEFFECT_MOVE: u32 = 2;

    struct ClipboardGuard;
    impl Drop for ClipboardGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseClipboard();
            }
        }
    }

    fn open_clipboard_with_retry() -> Result<ClipboardGuard, String> {
        for attempt in 0..10 {
            if unsafe { OpenClipboard(None) }.is_ok() {
                return Ok(ClipboardGuard);
            }
            if attempt < 9 {
                std::thread::sleep(Duration::from_millis(15));
            }
        }
        Err("System clipboard is busy; please try again".to_string())
    }

    fn preferred_drop_effect_format() -> u32 {
        let wide: Vec<u16> = OsStr::new("Preferred DropEffect")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        unsafe { RegisterClipboardFormatW(PCWSTR(wide.as_ptr())) }
    }

    fn alloc_hglobal(bytes: &[u8]) -> Result<HGLOBAL, String> {
        let h_mem = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len()) }
            .map_err(|e| format!("Failed to allocate clipboard memory: {e}"))?;
        let ptr = unsafe { GlobalLock(h_mem) };
        if ptr.is_null() {
            unsafe {
                let _ = GlobalFree(Some(h_mem));
            }
            return Err("Failed to lock clipboard memory".to_string());
        }
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr.cast::<u8>(), bytes.len());
            let _ = GlobalUnlock(h_mem);
        }
        Ok(h_mem)
    }

    fn decode_dropfiles_ansi(bytes: &[u8]) -> Vec<String> {
        const DROPFILES_SIZE: usize = 20;
        if bytes.len() < DROPFILES_SIZE {
            return Vec::new();
        }
        let p_files = u32::from_le_bytes(bytes[0..4].try_into().unwrap_or([0; 4])) as usize;
        if p_files < DROPFILES_SIZE || p_files > bytes.len() {
            return Vec::new();
        }
        let mut paths = Vec::new();
        for segment in bytes[p_files..].split(|&b| b == 0) {
            if segment.is_empty() {
                break;
            }
            let wide_len = unsafe {
                MultiByteToWideChar(CP_ACP, MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0), segment, None)
            };
            if wide_len > 0 {
                let mut wide = vec![0u16; wide_len as usize];
                let written = unsafe {
                    MultiByteToWideChar(
                        CP_ACP,
                        MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0),
                        segment,
                        Some(&mut wide),
                    )
                };
                if written > 0 {
                    paths.push(String::from_utf16_lossy(&wide[..written as usize]));
                    continue;
                }
            }
            paths.push(String::from_utf8_lossy(segment).into_owned());
        }
        paths
    }

    pub fn status() -> FileClipboardStatus {
        let sequence = unsafe { GetClipboardSequenceNumber() } as u64;
        let has_files = unsafe { IsClipboardFormatAvailable(CF_HDROP) }.is_ok();
        let mode = if has_files {
            recorded_mode_for(sequence)
        } else {
            None
        };
        FileClipboardStatus {
            supported: true,
            has_files,
            mode,
            sequence,
        }
    }

    pub fn write_files(paths: &[String], mode: FileClipboardMode) -> Result<u64, String> {
        let win_paths: Vec<String> = paths.iter().map(|p| to_windows_clipboard_path(p)).collect();
        let drop_bytes = encode_dropfiles_wide(&win_paths);
        let h_drop = alloc_hglobal(&drop_bytes)?;
        let effect = match mode {
            FileClipboardMode::Copy => DROPEFFECT_COPY,
            FileClipboardMode::Cut => DROPEFFECT_MOVE,
        };
        let h_effect = match alloc_hglobal(&effect.to_le_bytes()) {
            Ok(h) => h,
            Err(e) => {
                unsafe {
                    let _ = GlobalFree(Some(h_drop));
                }
                return Err(e);
            }
        };

        let _guard = match open_clipboard_with_retry() {
            Ok(g) => g,
            Err(e) => {
                unsafe {
                    let _ = GlobalFree(Some(h_drop));
                    let _ = GlobalFree(Some(h_effect));
                }
                return Err(e);
            }
        };

        if let Err(e) = unsafe { EmptyClipboard() } {
            unsafe {
                let _ = GlobalFree(Some(h_drop));
                let _ = GlobalFree(Some(h_effect));
            }
            return Err(format!("Failed to clear clipboard: {e}"));
        }

        if let Err(e) = unsafe { SetClipboardData(CF_HDROP, Some(HANDLE(h_drop.0))) } {
            unsafe {
                let _ = GlobalFree(Some(h_drop));
                let _ = GlobalFree(Some(h_effect));
            }
            return Err(format!("Failed to write files to clipboard: {e}"));
        }

        let cf_effect = preferred_drop_effect_format();
        if cf_effect == 0
            || unsafe { SetClipboardData(cf_effect, Some(HANDLE(h_effect.0))) }.is_err()
        {
            unsafe {
                let _ = GlobalFree(Some(h_effect));
            }
        }

        drop(_guard);
        Ok(unsafe { GetClipboardSequenceNumber() } as u64)
    }

    pub fn read_files() -> Result<Option<FileClipboardContent>, String> {
        if unsafe { IsClipboardFormatAvailable(CF_HDROP) }.is_err() {
            return Ok(None);
        }
        let _guard = open_clipboard_with_retry()?;
        let sequence = unsafe { GetClipboardSequenceNumber() } as u64;
        let Ok(handle) = (unsafe { GetClipboardData(CF_HDROP) }) else {
            return Ok(None);
        };
        let h_mem = HGLOBAL(handle.0);
        let size = unsafe { GlobalSize(h_mem) };
        if size == 0 {
            return Ok(None);
        }
        let ptr = unsafe { GlobalLock(h_mem) };
        if ptr.is_null() {
            return Err("Failed to read clipboard file list".to_string());
        }
        let bytes = unsafe { std::slice::from_raw_parts(ptr.cast::<u8>(), size) }.to_vec();
        unsafe {
            let _ = GlobalUnlock(h_mem);
        }

        let paths = decode_dropfiles_wide(&bytes).unwrap_or_else(|| decode_dropfiles_ansi(&bytes));
        if paths.is_empty() {
            return Ok(None);
        }

        let mut mode = FileClipboardMode::Copy;
        let cf_effect = preferred_drop_effect_format();
        if cf_effect != 0 && unsafe { IsClipboardFormatAvailable(cf_effect) }.is_ok() {
            if let Ok(eff_handle) = unsafe { GetClipboardData(cf_effect) } {
                let eff_mem = HGLOBAL(eff_handle.0);
                if unsafe { GlobalSize(eff_mem) } >= 4 {
                    let eff_ptr = unsafe { GlobalLock(eff_mem) };
                    if !eff_ptr.is_null() {
                        let raw = unsafe {
                            std::slice::from_raw_parts(eff_ptr.cast::<u8>(), 4)
                        };
                        let val = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
                        unsafe {
                            let _ = GlobalUnlock(eff_mem);
                        }
                        if (val & DROPEFFECT_MOVE != 0) && (val & DROPEFFECT_COPY == 0) {
                            mode = FileClipboardMode::Cut;
                        }
                    }
                }
            }
        }

        Ok(Some(FileClipboardContent {
            paths,
            mode,
            sequence,
        }))
    }

    pub fn clear_if_sequence(expected_sequence: Option<u64>) -> Result<(), String> {
        let current = unsafe { GetClipboardSequenceNumber() } as u64;
        if expected_sequence.is_some_and(|exp| exp != current) {
            return Ok(());
        }
        let _guard = open_clipboard_with_retry()?;
        let current = unsafe { GetClipboardSequenceNumber() } as u64;
        if expected_sequence.is_some_and(|exp| exp != current) {
            return Ok(());
        }
        let _ = unsafe { EmptyClipboard() };
        Ok(())
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use objc2::msg_send;
    use objc2::rc::{autoreleasepool, Retained};
    use objc2::runtime::AnyObject;
    use objc2_foundation::NSString;

    use super::{recorded_mode_for, FileClipboardContent, FileClipboardMode, FileClipboardStatus};

    fn pasteboard_change_count(pasteboard: *mut AnyObject) -> u64 {
        let count: isize = unsafe { msg_send![pasteboard, changeCount] };
        count.max(0) as u64
    }

    fn pasteboard_has_file_urls(pasteboard: *mut AnyObject) -> bool {
        unsafe {
            let types: *mut AnyObject = msg_send![pasteboard, types];
            if types.is_null() {
                return false;
            }
            let file_url = NSString::from_str("public.file-url");
            msg_send![types, containsObject: &*file_url]
        }
    }

    pub fn status() -> FileClipboardStatus {
        autoreleasepool(|_| unsafe {
            let pasteboard: *mut AnyObject =
                msg_send![objc2::class!(NSPasteboard), generalPasteboard];
            if pasteboard.is_null() {
                return FileClipboardStatus {
                    supported: true,
                    has_files: false,
                    mode: None,
                    sequence: 0,
                };
            }
            let sequence = pasteboard_change_count(pasteboard);
            let has_files = pasteboard_has_file_urls(pasteboard);
            let mode = if has_files {
                Some(recorded_mode_for(sequence).unwrap_or(FileClipboardMode::Copy))
            } else {
                None
            };
            FileClipboardStatus {
                supported: true,
                has_files,
                mode,
                sequence,
            }
        })
    }

    pub fn write_files(paths: &[String], _mode: FileClipboardMode) -> Result<u64, String> {
        autoreleasepool(|_| unsafe {
            let pasteboard: *mut AnyObject =
                msg_send![objc2::class!(NSPasteboard), generalPasteboard];
            if pasteboard.is_null() {
                return Err("System clipboard is unavailable".to_string());
            }
            let array: Option<Retained<AnyObject>> =
                msg_send![objc2::class!(NSMutableArray), arrayWithCapacity: paths.len()];
            let Some(array) = array else {
                return Err("Failed to prepare clipboard file list".to_string());
            };
            for path in paths {
                let ns_path = NSString::from_str(path);
                let url: Option<Retained<AnyObject>> =
                    msg_send![objc2::class!(NSURL), fileURLWithPath: &*ns_path];
                if let Some(url) = url {
                    let _: () = msg_send![&*array, addObject: &*url];
                }
            }
            let _: isize = msg_send![pasteboard, clearContents];
            let ok: bool = msg_send![pasteboard, writeObjects: &*array];
            if !ok {
                return Err("Failed to write files to clipboard".to_string());
            }
            Ok(pasteboard_change_count(pasteboard))
        })
    }

    pub fn read_files() -> Result<Option<FileClipboardContent>, String> {
        let has_files = autoreleasepool(|_| unsafe {
            let pasteboard: *mut AnyObject =
                msg_send![objc2::class!(NSPasteboard), generalPasteboard];
            !pasteboard.is_null() && pasteboard_has_file_urls(pasteboard)
        });
        if !has_files {
            return Ok(None);
        }
        #[cfg(all(feature = "gui", not(test)))]
        if !crate::clipboard_gate::probe_is_ready() {
            return Err("Clipboard source is not responding".to_string());
        }
        Ok(autoreleasepool(|_| unsafe {
            let pasteboard: *mut AnyObject =
                msg_send![objc2::class!(NSPasteboard), generalPasteboard];
            if pasteboard.is_null() {
                return None;
            }
            let sequence = pasteboard_change_count(pasteboard);
            let items: *mut AnyObject = msg_send![pasteboard, pasteboardItems];
            if items.is_null() {
                return None;
            }
            let file_url_type = NSString::from_str("public.file-url");
            let count: usize = msg_send![items, count];
            let mut paths = Vec::new();
            for index in 0..count {
                let item: *mut AnyObject = msg_send![items, objectAtIndex: index];
                let url: Option<Retained<NSString>> =
                    msg_send![item, stringForType: &*file_url_type];
                let Some(url) = url else { continue };
                let ns_url: *mut AnyObject =
                    msg_send![objc2::class!(NSURL), URLWithString: &*url];
                if ns_url.is_null() {
                    continue;
                }
                let file_url: *mut AnyObject = msg_send![ns_url, filePathURL];
                if file_url.is_null() {
                    continue;
                }
                let path: Option<Retained<NSString>> = msg_send![file_url, path];
                if let Some(path) = path {
                    paths.push(path.to_string());
                }
            }
            if paths.is_empty() {
                return None;
            }
            let mode = recorded_mode_for(sequence).unwrap_or(FileClipboardMode::Copy);
            Some(FileClipboardContent {
                paths,
                mode,
                sequence,
            })
        }))
    }

    pub fn clear_if_sequence(expected_sequence: Option<u64>) -> Result<(), String> {
        autoreleasepool(|_| unsafe {
            let pasteboard: *mut AnyObject =
                msg_send![objc2::class!(NSPasteboard), generalPasteboard];
            if pasteboard.is_null() {
                return Ok(());
            }
            let sequence = pasteboard_change_count(pasteboard);
            if expected_sequence.is_some_and(|exp| exp != sequence) {
                return Ok(());
            }
            let _: isize = msg_send![pasteboard, clearContents];
            Ok(())
        })
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod platform {
    use super::{FileClipboardContent, FileClipboardMode, FileClipboardStatus};

    pub fn status() -> FileClipboardStatus {
        FileClipboardStatus {
            supported: false,
            has_files: false,
            mode: None,
            sequence: 0,
        }
    }

    pub fn write_files(_paths: &[String], _mode: FileClipboardMode) -> Result<u64, String> {
        Err("System file clipboard is unsupported on this platform".to_string())
    }

    pub fn read_files() -> Result<Option<FileClipboardContent>, String> {
        Ok(None)
    }

    pub fn clear_if_sequence(_expected_sequence: Option<u64>) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_clipboard_path_normalizes_separators_and_prefixes() {
        assert_eq!(
            to_windows_clipboard_path(r"D:\projects\vlx-term/src/App.tsx"),
            r"D:\projects\vlx-term\src\App.tsx"
        );
        assert_eq!(
            to_windows_clipboard_path(r"\\?\C:\Users\me\文档.txt"),
            r"C:\Users\me\文档.txt"
        );
        assert_eq!(
            to_windows_clipboard_path(r"\\?\UNC\server\share\dir/file.txt"),
            r"\\server\share\dir\file.txt"
        );
        assert_eq!(to_windows_clipboard_path(r"C:\"), r"C:\");
        assert_eq!(to_windows_clipboard_path(r"D:\folder\"), r"D:\folder");
    }

    #[test]
    fn dropfiles_wide_round_trips_unicode_and_multiple_paths() {
        let input = vec![
            r"C:\Users\dev\文档.txt".to_string(),
            r"D:\work\folder\a b.rs".to_string(),
        ];
        let buf = encode_dropfiles_wide(&input);
        assert_eq!(u32::from_le_bytes(buf[0..4].try_into().unwrap()), 20);
        assert_eq!(u32::from_le_bytes(buf[16..20].try_into().unwrap()), 1);
        assert_eq!(decode_dropfiles_wide(&buf), Some(input));
    }

    #[test]
    fn dropfiles_wide_rejects_short_or_ansi_buffers() {
        assert_eq!(decode_dropfiles_wide(&[0u8; 10]), None);
        let mut ansi = vec![0u8; 24];
        ansi[0..4].copy_from_slice(&20u32.to_le_bytes());
        ansi[16..20].copy_from_slice(&0u32.to_le_bytes());
        assert_eq!(decode_dropfiles_wide(&ansi), None);
    }
}
