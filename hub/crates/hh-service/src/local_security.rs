//! The local admin secret belongs to the installer-selected Windows account.
use hh_core::Result;
#[cfg(windows)] use hh_core::Error;
use std::path::Path;
pub fn protect_admin_token(data:&Path,path:&Path)->Result<()> {
    #[cfg(not(windows))] {let _=data;use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(path,std::fs::Permissions::from_mode(0o600))?;Ok(())}
    #[cfg(windows)] {
        use std::os::windows::ffi::OsStrExt;
        let sid=std::fs::read_to_string(data.join("authorized-owner.sid"))?;let sid=sid.trim();
        if !sid.starts_with("S-1-5-")||sid.len()>128||!sid.bytes().all(|b|b.is_ascii_digit()||b==b'S'||b==b'-'){return Err(Error::BadRequest("invalid authorized Windows owner SID".into()));}
        #[link(name="advapi32")]extern "system" {fn ConvertStringSecurityDescriptorToSecurityDescriptorW(text:*const u16,revision:u32,descriptor:*mut *mut std::ffi::c_void,size:*mut u32)->i32;fn SetFileSecurityW(path:*const u16,info:u32,descriptor:*const std::ffi::c_void)->i32;}
        #[link(name="kernel32")]extern "system" {fn LocalFree(memory:*mut std::ffi::c_void)->*mut std::ffi::c_void;}
        let sddl:Vec<u16>=format!("D:P(A;;FA;;;SY)(A;;FA;;;BA)(A;;FR;;;{sid})").encode_utf16().chain(Some(0)).collect();
        let wide:Vec<u16>=path.as_os_str().encode_wide().chain(Some(0)).collect();let mut descriptor=std::ptr::null_mut();
        if unsafe{ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(),1,&mut descriptor,std::ptr::null_mut())}==0{return Err(std::io::Error::last_os_error().into());}
        let result=unsafe{SetFileSecurityW(wide.as_ptr(),4|0x80000000,descriptor)};let error=std::io::Error::last_os_error();unsafe{LocalFree(descriptor);}
        if result==0{return Err(error.into());}Ok(())
    }
}
