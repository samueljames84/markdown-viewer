use crate::Event;
use anyhow::{Context, Result};
use std::{
    ffi::OsStr,
    os::windows::ffi::OsStrExt,
    path::PathBuf,
    time::{Duration, Instant},
};
use tao::event_loop::EventLoopProxy;
use windows::{
    Win32::{
        Foundation::{
            COLORREF, CloseHandle, ERROR_ALREADY_EXISTS, ERROR_IO_PENDING, ERROR_PIPE_CONNECTED,
            GENERIC_READ, GENERIC_WRITE, GetLastError, GlobalFree, HANDLE, HLOCAL, HWND, LPARAM,
            LocalFree, WAIT_OBJECT_0, WPARAM,
        },
        Graphics::Gdi::{
            GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
        },
        Security::{
            Authorization::{
                ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
            },
            PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
        },
        Storage::FileSystem::{
            CreateFileW, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, FILE_SHARE_MODE,
            OPEN_EXISTING, PIPE_ACCESS_DUPLEX, ReadFile, WriteFile,
        },
        System::{
            DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData},
            IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED},
            LibraryLoader::GetModuleHandleW,
            Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock},
            Pipes::{
                ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE,
                PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
            },
            Registry::*,
            Threading::{CreateEventW, CreateMutexW, INFINITE, WaitForSingleObject},
        },
        UI::{
            Controls::Dialogs::{
                CC_FULLOPEN, CC_RGBINIT, CHOOSECOLORW, ChooseColorW, CommDlgExtendedError,
            },
            Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify, ShellExecuteW},
            WindowsAndMessaging::{
                ASFW_ANY, AllowSetForegroundWindow, FLASHW_TIMERNOFG, FLASHW_TRAY, FLASHWINFO,
                FlashWindowEx, GetForegroundWindow, ICON_BIG, ICON_SMALL, LoadIconW, SW_SHOWNORMAL,
                SendMessageW, WM_SETICON,
            },
        },
    },
    core::{HRESULT, PCWSTR, w},
};

pub fn wide(text: impl AsRef<OsStr>) -> Vec<u16> {
    text.as_ref().encode_wide().chain(Some(0)).collect()
}

pub fn launch(target: impl AsRef<OsStr>) -> Result<()> {
    let text = wide(target);
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(text.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    anyhow::ensure!(
        (result.0 as isize) > 32,
        "Windows could not open the requested target (code {})",
        result.0 as isize
    );
    Ok(())
}

pub fn color(hwnd: isize, hex: &str) -> Result<Option<String>> {
    anyhow::ensure!(hex.len() == 7 && hex.starts_with('#'), "Invalid color");
    let value = u32::from_str_radix(&hex[1..], 16)?;
    let rgb = ((value & 0xff) << 16) | (value & 0xff00) | ((value >> 16) & 0xff);
    let mut custom = [COLORREF(0); 16];
    let mut chooser = CHOOSECOLORW {
        lStructSize: std::mem::size_of::<CHOOSECOLORW>() as u32,
        hwndOwner: windows::Win32::Foundation::HWND(hwnd as _),
        rgbResult: COLORREF(rgb),
        lpCustColors: custom.as_mut_ptr(),
        Flags: CC_FULLOPEN | CC_RGBINIT,
        ..Default::default()
    };
    if unsafe { ChooseColorW(&mut chooser) }.as_bool() {
        let c = chooser.rgbResult.0;
        Ok(Some(format!(
            "#{:02x}{:02x}{:02x}",
            c & 255,
            (c >> 8) & 255,
            (c >> 16) & 255
        )))
    } else {
        let error = unsafe { CommDlgExtendedError() };
        anyhow::ensure!(error.0 == 0, "Color picker failed (code {error:?})");
        Ok(None)
    }
}

pub fn clipboard(hwnd: isize, text: &str) -> Result<()> {
    let value = wide(text);
    unsafe {
        OpenClipboard(Some(windows::Win32::Foundation::HWND(hwnd as _)))?;
        let result = (|| -> Result<()> {
            EmptyClipboard()?;
            let memory = GlobalAlloc(GMEM_MOVEABLE, value.len() * 2)?;
            let pointer = GlobalLock(memory);
            if pointer.is_null() {
                let _ = GlobalFree(Some(memory));
                anyhow::bail!("Could not allocate clipboard text");
            }
            std::ptr::copy_nonoverlapping(value.as_ptr(), pointer.cast::<u16>(), value.len());
            let _ = GlobalUnlock(memory);
            if let Err(error) = SetClipboardData(13, Some(HANDLE(memory.0))) {
                let _ = GlobalFree(Some(memory));
                return Err(error.into());
            }
            Ok(())
        })();
        let _ = CloseClipboard();
        result
    }
}

pub fn decorate(hwnd: isize) -> Result<()> {
    unsafe {
        let module = GetModuleHandleW(None)?;
        let icon = LoadIconW(Some(module.into()), PCWSTR(std::ptr::without_provenance(1)))?;
        for size in [ICON_SMALL, ICON_BIG] {
            SendMessageW(
                HWND(hwnd as _),
                WM_SETICON,
                Some(WPARAM(size as usize)),
                Some(LPARAM(icon.0 as isize)),
            );
        }
    }
    Ok(())
}

pub fn work_area(hwnd: isize) -> Result<(i32, i32, u32, u32)> {
    unsafe {
        let monitor = MonitorFromWindow(HWND(hwnd as _), MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        anyhow::ensure!(
            GetMonitorInfoW(monitor, &mut info).as_bool(),
            "Cannot determine monitor work area"
        );
        Ok((
            info.rcWork.left,
            info.rcWork.top,
            (info.rcWork.right - info.rcWork.left) as u32,
            (info.rcWork.bottom - info.rcWork.top) as u32,
        ))
    }
}

pub fn attention(hwnd: isize) {
    unsafe {
        if GetForegroundWindow() != HWND(hwnd as _) {
            let info = FLASHWINFO {
                cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
                hwnd: HWND(hwnd as _),
                dwFlags: FLASHW_TRAY | FLASHW_TIMERNOFG,
                uCount: 3,
                dwTimeout: 0,
            };
            let _ = FlashWindowEx(&info);
        }
    }
}
fn set_key(key: &str, name: &str, value: &OsStr) -> Result<()> {
    let mut handle = HKEY::default();
    let k = wide(key);
    let n = wide(name);
    let v = wide(value);
    unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(k.as_ptr()),
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut handle,
            None,
        )
        .ok()?;
        let bytes = std::slice::from_raw_parts(v.as_ptr().cast::<u8>(), v.len() * 2);
        let result = RegSetValueExW(handle, PCWSTR(n.as_ptr()), None, REG_SZ, Some(bytes)).ok();
        let _ = RegCloseKey(handle);
        result?;
    }
    Ok(())
}

fn read_key(key: &str) -> Result<Option<String>> {
    let k = wide(key);
    let mut bytes = vec![0u16; 32768];
    let mut size = (bytes.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(k.as_ptr()),
            None,
            RRF_RT_REG_SZ,
            None,
            Some(bytes.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if status == windows::Win32::Foundation::ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    status.ok()?;
    Ok(Some(String::from_utf16_lossy(
        &bytes[..(size as usize / 2).saturating_sub(1)],
    )))
}

pub fn association(register: bool) -> Result<()> {
    let exe = std::env::current_exe()?;
    let base = "Software\\Classes";
    let prog = format!("{base}\\MdViewer.Markdown");
    let app = format!("{base}\\Applications\\mdviewer.exe");
    let command = format!("\"{}\" \"%1\"", exe.display());
    if register {
        set_key(&prog, "", OsStr::new("Markdown document (MdViewer)"))?;
        set_key(
            &format!("{prog}\\DefaultIcon"),
            "",
            OsStr::new(&format!("\"{}\",0", exe.display())),
        )?;
        set_key(
            &format!("{prog}\\shell\\open\\command"),
            "",
            OsStr::new(&command),
        )?;
        set_key(&app, "FriendlyAppName", OsStr::new("MdViewer"))?;
        set_key(
            &format!("{app}\\shell\\open\\command"),
            "",
            OsStr::new(&command),
        )?;
        for ext in [".md", ".markdown", ".mdown"] {
            set_key(
                &format!("{base}\\{ext}\\OpenWithProgids"),
                "MdViewer.Markdown",
                OsStr::new(""),
            )?;
            set_key(&format!("{app}\\SupportedTypes"), ext, OsStr::new(""))?;
        }
    } else {
        anyhow::ensure!(
            read_key(&format!("{prog}\\shell\\open\\command"))?.is_none_or(|v| v == command),
            "The registration belongs to a different MdViewer location; it was not removed"
        );
        for ext in [".md", ".markdown", ".mdown"] {
            let key = wide(format!("{base}\\{ext}\\OpenWithProgids"));
            let name = wide("MdViewer.Markdown");
            let status = unsafe {
                RegDeleteKeyValueW(
                    HKEY_CURRENT_USER,
                    PCWSTR(key.as_ptr()),
                    PCWSTR(name.as_ptr()),
                )
            };
            if status != windows::Win32::Foundation::ERROR_FILE_NOT_FOUND {
                status.ok()?;
            }
        }
        for key in [&prog, &app] {
            if read_key(&format!("{key}\\shell\\open\\command"))?.is_none_or(|v| v == command) {
                let k = wide(key);
                let status = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(k.as_ptr())) };
                if status != windows::Win32::Foundation::ERROR_FILE_NOT_FOUND {
                    status.ok()?;
                }
            }
        }
    }
    unsafe {
        SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None);
    }
    if register {
        rfd::MessageDialog::new().set_title("MdViewer registered")
            .set_description("MdViewer is available in Open With.\nIn Windows Default Apps, choose MdViewer for .md, .markdown and .mdown. Windows does not permit apps to force this choice.").show();
        launch("ms-settings:defaultapps")?;
    } else {
        rfd::MessageDialog::new().set_title("Association removed").set_description("MdViewer's file registrations were removed. Other applications' registrations were not changed.").show();
    }
    Ok(())
}

pub struct Instance(HANDLE);
impl Drop for Instance {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

struct Security(PSECURITY_DESCRIPTOR);
impl Security {
    fn new() -> Result<Self> {
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                w!("D:P(A;;GA;;;SY)(A;;GA;;;OW)"),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )?;
        }
        Ok(Self(descriptor))
    }
    fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.0.0,
            bInheritHandle: false.into(),
        }
    }
}
impl Drop for Security {
    fn drop(&mut self) {
        unsafe {
            let _ = LocalFree(Some(HLOCAL(self.0.0)));
        }
    }
}

fn overlapped(
    handle: HANDLE,
    timeout: u32,
    operation: impl FnOnce(*mut OVERLAPPED, *mut u32) -> windows::core::Result<()>,
) -> Result<u32> {
    let event = Instance(unsafe { CreateEventW(None, true, false, None)? });
    let mut state = OVERLAPPED {
        hEvent: event.0,
        ..Default::default()
    };
    let mut count = 0;
    match operation(&mut state, &mut count) {
        Ok(()) => return Ok(count),
        Err(e) if e.code() == HRESULT::from_win32(ERROR_IO_PENDING.0) => {}
        Err(e) => return Err(e.into()),
    }
    unsafe {
        if WaitForSingleObject(event.0, timeout) != WAIT_OBJECT_0 {
            let _ = CancelIoEx(handle, Some(&state));
            let _ = GetOverlappedResult(handle, &state, &mut count, true);
            anyhow::bail!("Instance pipe operation timed out");
        }
        GetOverlappedResult(handle, &state, &mut count, false)?;
    }
    Ok(count)
}

fn bounded_read(handle: HANDLE, bytes: &mut [u8]) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut offset = 0;
    while offset < bytes.len() {
        let remaining = deadline
            .saturating_duration_since(Instant::now())
            .as_millis() as u32;
        anyhow::ensure!(remaining > 0, "Instance message timed out");
        let count = overlapped(handle, remaining, |state, count| unsafe {
            ReadFile(handle, Some(&mut bytes[offset..]), Some(count), Some(state))
        })?;
        anyhow::ensure!(count > 0, "Instance pipe was closed");
        offset += count as usize;
    }
    Ok(())
}

fn bounded_write(handle: HANDLE, bytes: &[u8]) -> Result<()> {
    let count = overlapped(handle, 5000, |state, count| unsafe {
        WriteFile(handle, Some(bytes), Some(count), Some(state))
    })?;
    anyhow::ensure!(
        count as usize == bytes.len(),
        "Instance message was not fully written"
    );
    Ok(())
}

pub fn instance(paths: Vec<PathBuf>, proxy: EventLoopProxy<Event>) -> Result<Option<Instance>> {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    std::env::var_os("APPDATA").hash(&mut hash);
    std::env::var_os("SESSIONNAME").hash(&mut hash);
    let id = hash.finish();
    let name = wide(format!("Local\\MdViewer-{id:x}"));
    let security = Security::new()?;
    let attributes = security.attributes();
    let mutex = unsafe { CreateMutexW(Some(&attributes), false, PCWSTR(name.as_ptr()))? };
    let exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    let pipe = format!("\\\\.\\pipe\\MdViewer-{id:x}");
    if exists {
        unsafe {
            let _ = AllowSetForegroundWindow(ASFW_ANY);
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        let pipe_name = wide(&pipe);
        let client = loop {
            match unsafe {
                CreateFileW(
                    PCWSTR(pipe_name.as_ptr()),
                    (GENERIC_READ | GENERIC_WRITE).0,
                    FILE_SHARE_MODE(0),
                    None,
                    OPEN_EXISTING,
                    FILE_FLAG_OVERLAPPED,
                    None,
                )
            } {
                Ok(handle) => break Instance(handle),
                Err(e) if Instant::now() < deadline => {
                    let _ = e;
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => {
                    unsafe {
                        let _ = CloseHandle(mutex);
                    }
                    return Err(e).context("The running instance did not accept the files");
                }
            }
        };
        let paths: Vec<Vec<u16>> = paths
            .iter()
            .map(|p| p.as_os_str().encode_wide().collect())
            .collect();
        let bytes = serde_json::to_vec(&paths)?;
        anyhow::ensure!(bytes.len() <= 1024 * 1024, "Too many command-line paths");
        bounded_write(client.0, &(bytes.len() as u32).to_le_bytes())?;
        bounded_write(client.0, &bytes)?;
        let mut ack = [0];
        bounded_read(client.0, &mut ack)?;
        anyhow::ensure!(ack == [1], "The running instance rejected the request");
        bounded_write(client.0, &[2])?;
        unsafe {
            let _ = CloseHandle(mutex);
        }
        return Ok(None);
    }
    std::thread::spawn(move || {
        let name = wide(&pipe);
        let security = match Security::new() {
            Ok(s) => s,
            Err(e) => {
                let _ = proxy.send_event(Event::Error(format!(
                    "Cannot secure single-instance pipe: {e}"
                )));
                return;
            }
        };
        let attributes = security.attributes();
        let access = PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE | FILE_FLAG_OVERLAPPED;
        let handle = unsafe {
            CreateNamedPipeW(
                PCWSTR(name.as_ptr()),
                access,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                4096,
                4096,
                5000,
                Some(&attributes),
            )
        };
        if handle.is_invalid() {
            let _ = proxy.send_event(Event::Error(
                "Cannot create the single-instance pipe".into(),
            ));
            return;
        }
        let _owned_pipe = Instance(handle);
        loop {
            let connected = overlapped(handle, INFINITE, |state, _| {
                match unsafe { ConnectNamedPipe(handle, Some(state)) } {
                    Err(e) if e.code() == HRESULT::from_win32(ERROR_PIPE_CONNECTED.0) => Ok(()),
                    result => result,
                }
            });
            if let Err(e) = connected {
                let _ = proxy.send_event(Event::Error(format!(
                    "Cannot accept instance connection: {e}"
                )));
                break;
            }
            let result = (|| -> Result<Vec<PathBuf>> {
                let mut length = [0; 4];
                bounded_read(handle, &mut length)?;
                let size = u32::from_le_bytes(length) as usize;
                anyhow::ensure!(size <= 1024 * 1024, "Oversized instance message");
                let mut bytes = vec![0; size];
                bounded_read(handle, &mut bytes)?;
                let paths: Vec<Vec<u16>> = serde_json::from_slice(&bytes)?;
                anyhow::ensure!(paths.len() <= 4096, "Too many forwarded files");
                use std::os::windows::ffi::OsStringExt;
                Ok(paths
                    .into_iter()
                    .map(|p| PathBuf::from(std::ffi::OsString::from_wide(&p)))
                    .collect())
            })();
            match result {
                Ok(paths) => {
                    if proxy.send_event(Event::Open(paths, true)).is_ok() {
                        let confirmation = (|| -> Result<()> {
                            bounded_write(handle, &[1])?;
                            let mut ack = [0];
                            bounded_read(handle, &mut ack)?;
                            anyhow::ensure!(ack == [2], "Invalid instance acknowledgement");
                            Ok(())
                        })();
                        if let Err(e) = confirmation {
                            let _ = proxy.send_event(Event::Error(format!(
                                "Instance acknowledgement failed: {e}"
                            )));
                        }
                    }
                }
                Err(e) => {
                    let _ = proxy.send_event(Event::Error(format!("Instance request failed: {e}")));
                }
            }
            if let Err(e) = unsafe { DisconnectNamedPipe(handle) } {
                let _ = proxy.send_event(Event::Error(format!(
                    "Cannot disconnect instance pipe: {e}"
                )));
                break;
            }
        }
    });
    Ok(Some(Instance(mutex)))
}
