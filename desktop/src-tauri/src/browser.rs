//! Untrusted webviews have separate WebView2 storage, no capabilities and no native commands.
use std::{collections::BTreeMap, net::IpAddr, sync::Mutex};
use serde::Serialize;
use tauri::{Manager, WebviewUrl, webview::{DownloadEvent, NewWindowResponse, PermissionKind, PermissionResponse, WebviewWindowBuilder}};
use crate::{AppState, store::{Result, now}};
#[derive(Clone,Serialize)] pub struct Tab {pub label:String,pub url:String,pub title:String,pub downloads:bool,pub permissions:Vec<String>}
#[derive(Default)] pub struct Browser {pub tabs:Mutex<BTreeMap<String,Tab>>}

pub fn public_url(input:&str)->Result<url::Url> {
    if input.len()>4096 {return Err("Address is too long".into());}
    let input=if input.contains("://") {input.to_owned()} else {format!("https://{input}")};
    let url=url::Url::parse(&input).map_err(|_|"Enter a valid website address")?;
    if url.scheme()!="https" || !url.username().is_empty() || url.password().is_some() || url.port().is_some_and(|v|v!=443) {return Err("Use a secure website address".into());}
    let host=url.host_str().ok_or("Enter a website address")?.trim_matches(['[',']']).trim_end_matches('.').to_lowercase();
    if let Ok(ip)=host.parse::<IpAddr>() {
        let blocked=match ip {
            IpAddr::V4(v)=>v.is_private()||v.is_loopback()||v.is_link_local()||v.is_unspecified()||v.is_multicast()||v.is_broadcast()||v.is_documentation()||v.octets()[0]==0,
            IpAddr::V6(v)=>v.is_loopback()||v.is_unspecified()||v.is_multicast()||(v.segments()[0]&0xfe00)==0xfc00||(v.segments()[0]&0xffc0)==0xfe80||v.to_ipv4_mapped().is_some(),
        }; if blocked {return Err("Local and private addresses are blocked in the internet browser".into());}
    } else if !host.contains('.') || [".localhost",".local",".lan",".internal",".home",".test",".invalid"].iter().any(|s|host.ends_with(s)) {return Err("Local addresses are blocked in the internet browser".into());}
    Ok(url)
}
pub async fn open(app:&tauri::AppHandle,input:&str)->Result<String> {
    let url=public_url(input)?;
    let state=app.state::<AppState>();
    if state.browser.tabs.lock().map_err(|_|"Browser is busy")?.len()>=12 {return Err("Close a tab before opening another".into());}
    let label=format!("browser-{}",ulid::Ulid::new());
    let tab=Tab {label:label.clone(),url:url.as_str().into(),title:url.host_str().unwrap_or("Website").into(),downloads:false,permissions:vec![]};
    state.browser.tabs.lock().map_err(|_|"Browser is busy")?.insert(label.clone(),tab.clone());
    let directory=state.store.root.join("internet-webviews"); crate::store::private_directory(&directory)?;
    let nav_app=app.clone(); let nav_label=label.clone();
    let title_app=app.clone(); let title_label=label.clone();
    let download_app=app.clone(); let download_label=label.clone();
    let builder=WebviewWindowBuilder::new(app,&label,WebviewUrl::External(url.clone()))
        .title(format!("Home Hub Browser · {}",url.host_str().unwrap_or("Website")))
        .inner_size(1100.0,760.0).min_inner_size(480.0,360.0)
        .data_directory(directory).devtools(false).disable_drag_drop_handler()
        .on_navigation(move |url| {
            if public_url(url.as_str()).is_err() {return false;}
            let state=nav_app.state::<AppState>();
            if let Ok(mut tabs)=state.browser.tabs.lock() {if let Some(tab)=tabs.get_mut(&nav_label) {tab.url=url.as_str().into();}}
            true
        })
        .on_new_window(|_,_|NewWindowResponse::Deny)
        .on_document_title_changed(move |_,title| {if let Ok(mut tabs)=title_app.state::<AppState>().browser.tabs.lock() {if let Some(tab)=tabs.get_mut(&title_label) {tab.title=title.chars().take(200).collect();}}})
        .on_permission_request(|webview,kind| {
            let permission=match kind {PermissionKind::Camera=>"camera",PermissionKind::Microphone=>"microphone",PermissionKind::Geolocation=>"location",PermissionKind::Notifications=>"notifications",PermissionKind::MediaKeySystemAccess=>"protected_media",_=>return PermissionResponse::Deny};
            let Ok(url)=webview.url() else {return PermissionResponse::Deny;};
            let origin=url.origin().ascii_serialization();
            let state=webview.app_handle().state::<AppState>();
            let allowed=state.browser.tabs.lock().ok().and_then(|tabs|tabs.get(webview.label()).cloned()).is_some_and(|t|t.permissions.contains(&format!("{origin}|{permission}")));
            if allowed {PermissionResponse::Allow} else {PermissionResponse::Deny}
        })
        .on_download(move |_,event| {
            let state=download_app.state::<AppState>();
            match event {
                DownloadEvent::Requested {url,destination}=> {
                    if public_url(url.as_str()).is_err() {return false;}
                    let allowed=state.browser.tabs.lock().ok().and_then(|t|t.get(&download_label).map(|t|t.downloads)).unwrap_or(false);
                    if !allowed {return false;}
                    let name=url.path_segments().and_then(|mut p|p.next_back()).unwrap_or("download").chars().filter(|c|c.is_ascii_alphanumeric()||".-_".contains(*c)).take(120).collect::<String>();
                    let id=ulid::Ulid::new().to_string();
                    let dir=state.store.root.join("downloads"); if crate::store::private_directory(&dir).is_err(){return false;}
                    *destination=dir.join(format!("{id}-{}",if name.is_empty(){"download"}else{&name}));
                    let body=serde_json::json!({"path":destination.to_string_lossy(),"status":"downloading","url":url.as_str(),"created_at":now()}).to_string();
                    state.store.save(Some(id),"download".into(),name,body).is_ok()
                },
                DownloadEvent::Finished {path,success,..}=> {
                    if let Some(path)=path {if let Some(name)=path.file_name().and_then(|s|s.to_str()) {let id=name.split('-').next().unwrap_or(""); if let Ok(rows)=state.store.list("download","",false) {if let Some(row)=rows.into_iter().find(|r|r.id==id) {if let Ok(mut body)=serde_json::from_str::<serde_json::Value>(&row.body) {body["status"]=serde_json::json!(if success {"complete"}else{"failed"}); let _=state.store.save(Some(row.id),row.kind,row.title,body.to_string());}}}}}
                    true
                },
                _=>false
            }
        });
    if builder.build().is_err() {state.browser.tabs.lock().map_err(|_|"Browser is busy")?.remove(&label);return Err("Could not open the browser. Install WebView2 or open the site in your default browser.".into());}
    state.store.save(None,"browser".into(),tab.title,serde_json::json!({"url":tab.url}).to_string())?;
    Ok(label)
}
#[cfg(test)] mod tests {use super::*; #[test] fn internet_navigation_rejects_local_and_native_schemes(){for u in ["http://example.com","file:///C:/secret","javascript:alert(1)","https://127.0.0.1","https://[::1]","https://10.0.0.1","https://192.168.1.2","https://user:password@example.com","https://tauri.localhost","https://printer.local","https://example.com:47801"] {assert!(public_url(u).is_err(),"{u}");} assert!(public_url("https://www.youtube.com").is_ok());}}
