use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;
const B64_CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
static ATTACHMENT_SEQ: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageAttachment {
    pub file_name: String,
    pub path: PathBuf,
    pub mime_type: String,
    pub size_bytes: usize,
    pub base64: String,
}

impl ImageAttachment {
    pub fn data_url(&self) -> String {
        format!("data:{};base64,{}", self.mime_type, self.base64)
    }

    pub fn summary_label(&self) -> String {
        let kb = self.size_bytes.div_ceil(1024);
        format!("{} ({} КБ)", self.file_name, kb)
    }
}

pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        out.push(B64_CHARS[(b0 >> 2) as usize] as char);
        out.push(B64_CHARS[(((b0 & 3) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(B64_CHARS[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(B64_CHARS[(b2 & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

pub fn detect_mime(path: &Path, bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return "image/png";
    }
    if bytes.starts_with(b"\xff\xd8\xff") {
        return "image/jpeg";
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return "image/gif";
    }
    if bytes.starts_with(b"BM") {
        return "image/bmp";
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return "image/webp";
    }
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "gif" => "image/gif",
        _ => "application/octet-stream",
    }
}

pub fn load_image_file(path: &Path) -> io::Result<ImageAttachment> {
    if !path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("File not found: {}", path.display()),
        ));
    }
    let bytes = fs::read(path)?;
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Image exceeds 20 MB limit: {} bytes", bytes.len()),
        ));
    }
    let mime = detect_mime(path, &bytes);
    if mime == "application/octet-stream" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Unsupported image format. Allowed: PNG, JPG, WEBP, BMP, GIF.",
        ));
    }
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("image")
        .to_string();
    let base64 = base64_encode(&bytes);
    let size_bytes = bytes.len();
    Ok(ImageAttachment {
        file_name,
        path: path.to_path_buf(),
        mime_type: mime.to_string(),
        size_bytes,
        base64,
    })
}

#[allow(dead_code)]
pub fn scan_mmproj_files(models_dir: &Path) -> io::Result<Vec<String>> {
    let Ok(entries) = fs::read_dir(models_dir) else {
        return Ok(Vec::new());
    };
    let mut list: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().into_string().ok()?;
            let l = n.to_ascii_lowercase();
            (l.contains("mmproj") && l.ends_with(".gguf")).then_some(n)
        })
        .collect();
    list.sort();
    Ok(list)
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn dib_to_bmp(dib: &[u8]) -> io::Result<Vec<u8>> {
    if dib.len() < 40 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "DIB header too short",
        ));
    }
    let bi_size = u32::from_le_bytes(dib[0..4].try_into().unwrap()) as usize;
    if bi_size < 40 || bi_size > dib.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid DIB header size",
        ));
    }
    let bit_count = u16::from_le_bytes(dib[14..16].try_into().unwrap());
    let bi_compression = u32::from_le_bytes(dib[16..20].try_into().unwrap());
    let clr_used = u32::from_le_bytes(dib[32..36].try_into().unwrap()) as usize;

    let palette_colors = if clr_used > 0 {
        clr_used
    } else if bit_count <= 8 {
        1usize << bit_count
    } else {
        0
    };
    let bitfields_size = if bi_compression == 3 && bi_size == 40 {
        12
    } else {
        0
    };
    let palette_bytes = palette_colors
        .checked_mul(4)
        .and_then(|v| v.checked_add(bitfields_size))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "DIB palette overflow"))?;
    let pixel_offset = bi_size
        .checked_add(palette_bytes)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "DIB offset overflow"))?;
    if pixel_offset > dib.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid DIB pixel data offset",
        ));
    }
    let off_bits = u32::try_from(14usize.saturating_add(pixel_offset))
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "DIB offset exceeds u32"))?;
    let file_size = u32::try_from(14usize.saturating_add(dib.len()))
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "DIB size exceeds u32"))?;

    let mut bmp = Vec::with_capacity(14 + dib.len());
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&file_size.to_le_bytes());
    bmp.extend_from_slice(&[0u8; 4]); // reserved
    bmp.extend_from_slice(&off_bits.to_le_bytes());
    bmp.extend_from_slice(dib);
    Ok(bmp)
}

#[cfg(windows)]
pub fn open_in_explorer(path: &Path) {
    let target = if path.exists() {
        path.to_path_buf()
    } else if let Some(p) = path.parent() {
        p.to_path_buf()
    } else {
        path.to_path_buf()
    };
    if target.is_file() {
        let _ = std::process::Command::new("explorer.exe")
            .arg(format!("/select,{}", target.display()))
            .spawn();
    } else {
        let _ = std::process::Command::new("explorer.exe")
            .arg(&target)
            .spawn();
    }
}

#[cfg(not(windows))]
pub fn open_in_explorer(path: &Path) {
    let _ = path;
}

#[cfg(windows)]
pub fn pick_image_file_dialog() -> Option<PathBuf> {
    use std::ptr;

    #[repr(C)]
    struct OpenFileNameW {
        l_struct_size: u32,
        hwnd_owner: *mut core::ffi::c_void,
        h_instance: *mut core::ffi::c_void,
        lpstr_filter: *const u16,
        lpstr_custom_filter: *mut u16,
        n_max_cust_filter: u32,
        n_filter_index: u32,
        lpstr_file: *mut u16,
        n_max_file: u32,
        lpstr_file_title: *mut u16,
        n_max_file_title: u32,
        lpstr_initial_dir: *const u16,
        lpstr_title: *const u16,
        flags: u32,
        n_file_offset: u16,
        n_file_extension: u16,
        lpstr_def_ext: *const u16,
        l_cust_data: isize,
        lpfn_hook: *const core::ffi::c_void,
        lp_template_name: *const u16,
        pv_reserved: *mut core::ffi::c_void,
        dw_reserved: u32,
        flags_ex: u32,
    }

    #[link(name = "comdlg32")]
    unsafe extern "system" {
        fn GetOpenFileNameW(lpofn: *mut OpenFileNameW) -> i32;
    }

    const OFN_FILEMUSTEXIST: u32 = 0x0000_1000;
    const OFN_PATHMUSTEXIST: u32 = 0x0000_0800;
    const OFN_NOCHANGEDIR: u32 = 0x0000_0008;
    const OFN_EXPLORER: u32 = 0x0008_0000;

    let filter_str = "Images (*.png;*.jpg;*.jpeg;*.webp;*.bmp;*.gif)\0*.png;*.jpg;*.jpeg;*.webp;*.bmp;*.gif\0All Files (*.*)\0*.*\0\0";
    let filter_w: Vec<u16> = filter_str.encode_utf16().collect();
    let title_w: Vec<u16> = "Выберите изображение\0".encode_utf16().collect();
    let mut file_buf = vec![0u16; 1024];

    unsafe {
        let mut ofn = OpenFileNameW {
            l_struct_size: std::mem::size_of::<OpenFileNameW>() as u32,
            hwnd_owner: ptr::null_mut(),
            h_instance: ptr::null_mut(),
            lpstr_filter: filter_w.as_ptr(),
            lpstr_custom_filter: ptr::null_mut(),
            n_max_cust_filter: 0,
            n_filter_index: 1,
            lpstr_file: file_buf.as_mut_ptr(),
            n_max_file: file_buf.len() as u32,
            lpstr_file_title: ptr::null_mut(),
            n_max_file_title: 0,
            lpstr_initial_dir: ptr::null(),
            lpstr_title: title_w.as_ptr(),
            flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR | OFN_EXPLORER,
            n_file_offset: 0,
            n_file_extension: 0,
            lpstr_def_ext: ptr::null(),
            l_cust_data: 0,
            lpfn_hook: ptr::null(),
            lp_template_name: ptr::null(),
            pv_reserved: ptr::null_mut(),
            dw_reserved: 0,
            flags_ex: 0,
        };
        if GetOpenFileNameW(&mut ofn) != 0 {
            let len = file_buf.iter().position(|&c| c == 0).unwrap_or(0);
            if len > 0 {
                let s = String::from_utf16_lossy(&file_buf[..len]);
                return Some(PathBuf::from(s));
            }
        }
    }
    None
}

#[cfg(not(windows))]
pub fn pick_image_file_dialog() -> Option<PathBuf> {
    None
}

#[cfg_attr(not(windows), allow(dead_code))]
fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn save_attachment(bytes: &[u8], ext: &str, attachments_dir: &Path) -> io::Result<PathBuf> {
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Image attachment exceeds maximum allowed size",
        ));
    }
    fs::create_dir_all(attachments_dir)?;
    let ts = now_ts();
    let seq = ATTACHMENT_SEQ.fetch_add(1, Ordering::Relaxed);
    let name = format!("clip_{ts}_{seq}.{ext}");
    let path = attachments_dir.join(name);
    fs::write(&path, bytes)?;
    Ok(path)
}

#[cfg(windows)]
pub fn read_clipboard_image(attachments_dir: &Path) -> io::Result<Option<ImageAttachment>> {
    use std::ptr;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
        RegisterClipboardFormatW,
    };
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

    const CF_DIB: u32 = 8;
    const CF_HDROP: u32 = 15;

    let mut opened = false;
    for _ in 0..5 {
        if unsafe { OpenClipboard(ptr::null_mut()) } != 0 {
            opened = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    if !opened {
        return Ok(None);
    }

    unsafe {
        let png_name: Vec<u16> = "PNG\0".encode_utf16().collect();
        let cf_png = RegisterClipboardFormatW(png_name.as_ptr());
        if cf_png != 0 && IsClipboardFormatAvailable(cf_png) != 0 {
            let handle = GetClipboardData(cf_png);
            if !handle.is_null() {
                let ptr = GlobalLock(handle);
                if !ptr.is_null() {
                    let size = GlobalSize(handle);
                    let png_slice = std::slice::from_raw_parts(ptr as *const u8, size);
                    if png_slice.starts_with(b"\x89PNG\r\n\x1a\n") {
                        let bytes = png_slice.to_vec();
                        GlobalUnlock(handle);
                        CloseClipboard();
                        let saved_path = save_attachment(&bytes, "png", attachments_dir)?;
                        let file_name = saved_path
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("clip.png")
                            .to_string();
                        let base64 = base64_encode(&bytes);
                        let size_bytes = bytes.len();
                        return Ok(Some(ImageAttachment {
                            file_name,
                            path: saved_path,
                            mime_type: "image/png".to_string(),
                            size_bytes,
                            base64,
                        }));
                    }
                    GlobalUnlock(handle);
                }
            }
        }

        if IsClipboardFormatAvailable(CF_DIB) != 0 {
            let handle = GetClipboardData(CF_DIB);
            if !handle.is_null() {
                let ptr = GlobalLock(handle);
                if !ptr.is_null() {
                    let size = GlobalSize(handle);
                    let dib_slice = std::slice::from_raw_parts(ptr as *const u8, size);
                    let bmp_res = dib_to_bmp(dib_slice);
                    GlobalUnlock(handle);
                    if let Ok(bytes) = bmp_res {
                        CloseClipboard();
                        let saved_path = save_attachment(&bytes, "bmp", attachments_dir)?;
                        let file_name = saved_path
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("clip.bmp")
                            .to_string();
                        let base64 = base64_encode(&bytes);
                        let size_bytes = bytes.len();
                        return Ok(Some(ImageAttachment {
                            file_name,
                            path: saved_path,
                            mime_type: "image/bmp".to_string(),
                            size_bytes,
                            base64,
                        }));
                    }
                }
            }
        }

        if IsClipboardFormatAvailable(CF_HDROP) != 0 {
            let handle = GetClipboardData(CF_HDROP);
            if !handle.is_null() {
                let ptr = GlobalLock(handle);
                if !ptr.is_null() {
                    let size = GlobalSize(handle);
                    if size >= 20 {
                        let p_files = *(ptr as *const u32) as usize;
                        let f_wide = *((ptr as *const u8).add(16) as *const i32) != 0;
                        if p_files >= 20 && p_files < size {
                            let raw_bytes =
                                std::slice::from_raw_parts((ptr as *const u8).add(p_files), size - p_files);
                            let file_path = if f_wide {
                                let mut units = Vec::new();
                                for chunk in raw_bytes.chunks_exact(2) {
                                    let code = u16::from_le_bytes([chunk[0], chunk[1]]);
                                    if code == 0 {
                                        break;
                                    }
                                    units.push(code);
                                }
                                String::from_utf16_lossy(&units)
                            } else {
                                let a_slice = raw_bytes;
                                a_slice
                                    .iter()
                                    .position(|&c| c == 0)
                                    .map(|p| String::from_utf8_lossy(&a_slice[..p]).to_string())
                                    .unwrap_or_default()
                            };
                            GlobalUnlock(handle);
                            if !file_path.is_empty() {
                                let p = Path::new(&file_path);
                                if p.is_file()
                                    && let Ok(img) = load_image_file(p)
                                {
                                    CloseClipboard();
                                    return Ok(Some(img));
                                }
                            }
                        } else {
                            GlobalUnlock(handle);
                        }
                    } else {
                        GlobalUnlock(handle);
                    }
                }
            }
        }

        CloseClipboard();
        Ok(None)
    }
}

#[allow(dead_code)]
#[cfg(windows)]
pub fn read_clipboard_text() -> io::Result<Option<String>> {
    use std::ptr;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    };
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

    const CF_UNICODETEXT: u32 = 13;
    let mut opened = false;
    for _ in 0..5 {
        if unsafe { OpenClipboard(ptr::null_mut()) } != 0 {
            opened = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    if !opened {
        return Ok(None);
    }
    unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 {
            CloseClipboard();
            return Ok(None);
        }
        let handle = GetClipboardData(CF_UNICODETEXT);
        if handle.is_null() {
            CloseClipboard();
            return Ok(None);
        }
        let ptr = GlobalLock(handle);
        if ptr.is_null() {
            CloseClipboard();
            return Ok(None);
        }
        let size = GlobalSize(handle) / 2;
        let slice = std::slice::from_raw_parts(ptr as *const u16, size);
        let len = slice.iter().position(|&c| c == 0).unwrap_or(size);
        let text = String::from_utf16_lossy(&slice[..len]);
        GlobalUnlock(handle);
        CloseClipboard();
        if text.is_empty() {
            Ok(None)
        } else {
            Ok(Some(text))
        }
    }
}

#[allow(dead_code)]
#[cfg(windows)]
pub fn write_clipboard_text(text: &str) -> io::Result<()> {
    use std::ptr;
    use windows_sys::Win32::Foundation::GlobalFree;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock,
    };

    const CF_UNICODETEXT: u32 = 13;
    let mut opened = false;
    for _ in 0..5 {
        if unsafe { OpenClipboard(ptr::null_mut()) } != 0 {
            opened = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    if !opened {
        return Err(io::Error::other("Failed to open clipboard"));
    }
    let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes_len = utf16.len() * 2;
    unsafe {
        EmptyClipboard();
        let h_mem = GlobalAlloc(GMEM_MOVEABLE, bytes_len);
        if h_mem.is_null() {
            CloseClipboard();
            return Err(io::Error::other("Failed to allocate clipboard memory"));
        }
        let p_mem = GlobalLock(h_mem);
        if p_mem.is_null() {
            GlobalFree(h_mem);
            CloseClipboard();
            return Err(io::Error::other("Failed to lock clipboard memory"));
        }
        ptr::copy_nonoverlapping(utf16.as_ptr() as *const u8, p_mem as *mut u8, bytes_len);
        GlobalUnlock(h_mem);
        if SetClipboardData(CF_UNICODETEXT, h_mem).is_null() {
            GlobalFree(h_mem);
            CloseClipboard();
            return Err(io::Error::other("Failed to set clipboard data"));
        }
        CloseClipboard();
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn read_clipboard_image(_attachments_dir: &Path) -> io::Result<Option<ImageAttachment>> {
    Ok(None)
}

#[allow(dead_code)]
#[cfg(not(windows))]
pub fn read_clipboard_text() -> io::Result<Option<String>> {
    Ok(None)
}

#[allow(dead_code)]
#[cfg(not(windows))]
pub fn write_clipboard_text(_text: &str) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_base64_correctly() {
        assert_eq!(base64_encode(b"hello"), "aGVsbG8=");
        assert_eq!(base64_encode(b"world!"), "d29ybGQh");
        assert_eq!(base64_encode(b""), "");
    }

    #[test]
    fn detects_mime_signatures() {
        assert_eq!(
            detect_mime(Path::new("test.png"), b"\x89PNG\r\n\x1a\n"),
            "image/png"
        );
        assert_eq!(
            detect_mime(Path::new("test.jpg"), b"\xff\xd8\xff"),
            "image/jpeg"
        );
        assert_eq!(detect_mime(Path::new("test.bmp"), b"BM123"), "image/bmp");
    }

    #[test]
    fn converts_synthetic_dib_to_bmp() {
        let mut dib = vec![0u8; 40];
        dib[0..4].copy_from_slice(&40u32.to_le_bytes()); // biSize
        dib[14..16].copy_from_slice(&24u16.to_le_bytes()); // 24 bpp
        let bmp = dib_to_bmp(&dib).unwrap();
        assert_eq!(&bmp[0..2], b"BM");
        assert_eq!(bmp.len(), 54);
    }
}
