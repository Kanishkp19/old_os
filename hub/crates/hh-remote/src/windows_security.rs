//! Fail-closed authentication for the installed service/session pipe pair.
//! The install directory must be administrator-owned (the installer ACL).
use std::{ffi::c_void, path::PathBuf, ptr};
type Handle = *mut c_void;
#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
    fn CloseHandle(handle: Handle) -> i32;
    fn QueryFullProcessImageNameW(handle: Handle, flags: u32, name: *mut u16, size: *mut u32) -> i32;
    fn GetCurrentProcessId() -> u32;
    fn ProcessIdToSessionId(pid: u32, session: *mut u32) -> i32;
    fn WTSGetActiveConsoleSessionId() -> u32;
    fn GetNamedPipeClientProcessId(pipe: Handle, pid: *mut u32) -> i32;
    fn GetNamedPipeServerProcessId(pipe: Handle, pid: *mut u32) -> i32;
    fn LocalFree(mem: Handle) -> Handle;
    fn WaitForSingleObject(object: Handle, milliseconds: u32) -> u32;
}
#[link(name = "advapi32")]
extern "system" {
    fn OpenProcessToken(process: Handle, access: u32, token: *mut Handle) -> i32;
    fn GetTokenInformation(token: Handle, class: u32, info: Handle, len: u32, needed: *mut u32) -> i32;
    fn ConvertSidToStringSidW(sid: Handle, text: *mut *mut u16) -> i32;
    fn ConvertStringSecurityDescriptorToSecurityDescriptorW(text: *const u16, revision: u32, descriptor: *mut Handle, size: *mut u32) -> i32;
}
#[link(name = "wtsapi32")]
extern "system" { fn WTSQueryUserToken(session: u32, token: *mut Handle) -> i32; }
struct OwnedHandle(Handle);
impl Drop for OwnedHandle { fn drop(&mut self) { unsafe { CloseHandle(self.0); } } }
fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain(Some(0)).collect() }
fn token_sid(token: Handle) -> Option<String> {
    unsafe {
        let mut needed = 0;
        GetTokenInformation(token, 1, ptr::null_mut(), 0, &mut needed);
        if needed == 0 || needed > 65536 { return None; }
        let mut buffer = vec![0u8; needed as usize];
        if GetTokenInformation(token, 1, buffer.as_mut_ptr().cast(), needed, &mut needed) == 0 { return None; }
        let sid = ptr::read_unaligned(buffer.as_ptr().cast::<Handle>());
        let mut text = ptr::null_mut();
        if ConvertSidToStringSidW(sid, &mut text) == 0 || text.is_null() { return None; }
        let mut n = 0;
        while n < 256 && *text.add(n) != 0 { n += 1; }
        let result = if n < 256 { String::from_utf16(std::slice::from_raw_parts(text, n)).ok() } else { None };
        LocalFree(text.cast());
        result
    }
}
fn session_of(pid: u32) -> Option<u32> {
    let mut session = 0;
    (unsafe { ProcessIdToSessionId(pid, &mut session) } != 0).then_some(session)
}
/// Pipe names are scoped to the interactive session; user logoff cannot
/// leave another user's helper silently serving later requests.
pub fn current_pipe_name() -> Option<String> {
    let session = session_of(unsafe { GetCurrentProcessId() })?;
    (session != 0).then(|| format!(r"\\.\pipe\homehub-session-{session}"))
}
pub fn active_pipe_name() -> Option<String> {
    let session = unsafe { WTSGetActiveConsoleSessionId() };
    (session != u32::MAX && session != 0).then(|| format!(r"\\.\pipe\homehub-session-{session}"))
}
fn peer_is_installed(pid: u32, binary: &str, service: bool) -> bool {
    unsafe {
        let process = OpenProcess(0x1000, 0, pid); // PROCESS_QUERY_LIMITED_INFORMATION
        if process.is_null() { return false; }
        let process = OwnedHandle(process);
        let mut name = vec![0u16; 32768];
        let mut len = name.len() as u32;
        if QueryFullProcessImageNameW(process.0, 0, name.as_mut_ptr(), &mut len) == 0 || len as usize > name.len() { return false; }
        let Ok(image) = String::from_utf16(&name[..len as usize]) else { return false; };
        let Some(expected) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join(binary))) else { return false; };
        let (Ok(expected), Ok(image)) = (expected.canonicalize(), PathBuf::from(image).canonicalize()) else { return false; };
        if expected.as_os_str().to_string_lossy().to_lowercase() != image.as_os_str().to_string_lossy().to_lowercase() { return false; }
        let mut token = ptr::null_mut();
        if OpenProcessToken(process.0, 8, &mut token) == 0 { return false; } // TOKEN_QUERY
        let token = OwnedHandle(token);
        let Some(sid) = token_sid(token.0) else { return false; };
        if service { return sid == "S-1-5-18" && session_of(pid) == Some(0); }
        let session = WTSGetActiveConsoleSessionId();
        if session == u32::MAX || session == 0 || session_of(pid) != Some(session) { return false; }
        let mut owner = ptr::null_mut();
        if WTSQueryUserToken(session, &mut owner) == 0 { return false; }
        let owner = OwnedHandle(owner);
        token_sid(owner.0).is_some_and(|owner| owner == sid)
    }
}
/// Authenticate the client as the exact installed LocalSystem service.
pub fn verify_service_client(pipe: Handle) -> bool {
    let mut pid = 0;
    unsafe { GetNamedPipeClientProcessId(pipe, &mut pid) != 0 && pid != 0 && peer_is_installed(pid, "hh-service.exe", true) }
}
/// Authenticate the server as the exact installed helper owned by the
/// current active-console user. Query failures deny access.
pub fn verify_session_server(pipe: Handle) -> bool {
    let mut pid = 0;
    unsafe { GetNamedPipeServerProcessId(pipe, &mut pid) != 0 && pid != 0 && peer_is_installed(pid, "hh-session.exe", false) }
}
#[repr(C)]
pub struct SecurityAttributes { length: u32, descriptor: Handle, inherit: i32 }
/// Explicit protected pipe DACL grants only LocalSystem client access.
/// The helper's creating handle is retained; no Administrators/Everyone ACE.
pub struct PipeSecurity { descriptor: Handle }
impl PipeSecurity {
    pub fn new() -> std::io::Result<Self> {
        let mut descriptor = ptr::null_mut();
        if unsafe { ConvertStringSecurityDescriptorToSecurityDescriptorW(wide("D:P(A;;GA;;;SY)").as_ptr(), 1, &mut descriptor, ptr::null_mut()) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Self { descriptor })
    }
    pub fn attributes(&self) -> SecurityAttributes {
        SecurityAttributes { length: std::mem::size_of::<SecurityAttributes>() as u32, descriptor: self.descriptor, inherit: 0 }
    }
}
impl Drop for PipeSecurity { fn drop(&mut self) { unsafe { LocalFree(self.descriptor); } } }

/// Keep a real process handle to detect service exit (PID reuse is harmless).
pub struct ServiceLifetime(OwnedHandle);
impl ServiceLifetime {
    pub fn from_pipe(pipe: Handle) -> Option<Self> {
        let mut pid = 0;
        if unsafe { GetNamedPipeClientProcessId(pipe, &mut pid) } == 0 || pid == 0 { return None; }
        let handle = unsafe { OpenProcess(0x00100000, 0, pid) }; // SYNCHRONIZE
        (!handle.is_null()).then(|| Self(OwnedHandle(handle)))
    }
    pub fn is_alive(&self) -> bool { unsafe { WaitForSingleObject(self.0.0, 0) == 258 } } // WAIT_TIMEOUT
}
