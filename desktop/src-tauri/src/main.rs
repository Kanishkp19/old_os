#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod browser;
mod proxy;
mod store;
use tauri::{Emitter, Manager, WebviewWindow, WebviewWindowBuilder, WebviewUrl};
use store::{Result, Record, Store};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

pub struct AppState {store:Store, hub:proxy::Hub, browser:browser::Browser, ready:std::sync::atomic::AtomicBool, saving:std::sync::Mutex<std::collections::BTreeMap<String,std::sync::Arc<std::sync::atomic::AtomicBool>>>}
fn trusted(window:&WebviewWindow)->Result<()> {
    let url=window.url().map_err(|_|"Cannot identify app window")?;
    let local=url.scheme()=="tauri"&&url.host_str()==Some("localhost") || matches!(url.scheme(),"http"|"https")&&url.host_str()==Some("tauri.localhost") || cfg!(debug_assertions)&&url.scheme()=="http"&&url.host_str()==Some("127.0.0.1")&&url.port()==Some(1420);
    if window.label()!="main" || !local {return Err("This command is only available in Home Hub".into());}
    Ok(())
}
#[tauri::command] async fn hub_request(window:WebviewWindow,state:tauri::State<'_,AppState>,path:String,method:String,body:Option<String>)->Result<proxy::Reply> {trusted(&window)?;state.hub.reply(&path,&method,body).await}
#[tauri::command] fn local_list(window:WebviewWindow,state:tauri::State<'_,AppState>,kind:String,query:String,trash:bool)->Result<Vec<Record>> {trusted(&window)?;state.store.list(&kind,&query,trash)}
#[tauri::command] fn local_save(window:WebviewWindow,state:tauri::State<'_,AppState>,id:Option<String>,kind:String,title:String,body:String)->Result<Record> {trusted(&window)?;state.store.save(id,kind,title,body)}
#[tauri::command] fn local_trash(window:WebviewWindow,state:tauri::State<'_,AppState>,id:String)->Result<()> {trusted(&window)?;state.store.trash(&id,false)}
#[tauri::command] fn local_restore(window:WebviewWindow,state:tauri::State<'_,AppState>,id:String)->Result<()> {trusted(&window)?;state.store.trash(&id,true)}
#[tauri::command] fn local_purge(window:WebviewWindow,state:tauri::State<'_,AppState>,id:String)->Result<()> {trusted(&window)?;state.store.purge(&id)}
#[tauri::command] async fn local_export(window:WebviewWindow,state:tauri::State<'_,AppState>)->Result<bool> {
    trusted(&window)?; let text=state.store.export()?;
    tauri::async_runtime::spawn_blocking(move || {let Some(path)=rfd::FileDialog::new().set_file_name("home-hub-private.json").add_filter("Home Hub app data",&["json"]).save_file() else {return Ok(false);};write_export(&path,text.as_bytes())?;Ok(true)}).await.map_err(|_|"Export was interrupted")?
}
#[tauri::command] async fn local_import(window:WebviewWindow,app:tauri::AppHandle)->Result<Option<usize>> {
    trusted(&window)?;
    tauri::async_runtime::spawn_blocking(move || {let Some(path)=rfd::FileDialog::new().add_filter("Home Hub app data",&["json"]).pick_file() else {return Ok(None);};let text=read_text(&path,8*1024*1024)?;app.state::<AppState>().store.import(&text).map(Some)}).await.map_err(|_|"Import was interrupted")?
}
#[tauri::command] async fn note_export(window:WebviewWindow,app:tauri::AppHandle,id:String,format:String)->Result<bool> {
    trusted(&window)?;store::validate_id(&id)?;
    if !["md","txt"].contains(&format.as_str()){return Err("Choose Markdown or text".into());}
    let state=app.state::<AppState>();
    let row=state.store.list("note","",false)?.into_iter().chain(state.store.list("note","",true)?).find(|r|r.id==id).ok_or("Note no longer exists")?;
    let title=row.title.chars().filter(|c|c.is_ascii_alphanumeric()||" -_".contains(*c)).take(100).collect::<String>();
    let name=format!("{}.{}",if title.is_empty(){"note"}else{&title},format);
    tauri::async_runtime::spawn_blocking(move || {let Some(path)=rfd::FileDialog::new().set_file_name(&name).add_filter("Note",&[format.as_str()]).save_file() else {return Ok(false);};write_export(&path,row.body.as_bytes())?;Ok(true)}).await.map_err(|_|"Export interrupted")?
}
#[tauri::command] async fn note_import(window:WebviewWindow,app:tauri::AppHandle)->Result<Option<Record>> {
    trusted(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let Some(path)=rfd::FileDialog::new().add_filter("Markdown or text",&["md","txt","markdown"]).pick_file() else {return Ok(None);};
        let meta=std::fs::metadata(&path).map_err(|_|"Could not read note")?;
        if !meta.is_file()||meta.len()>1024*1024 {return Err("Note exceeds 1 MB".into());}
        let body=read_text(&path,1024*1024)?;
        let title=path.file_stem().and_then(|s|s.to_str()).unwrap_or("Imported note").chars().take(120).collect();
        app.state::<AppState>().store.save(None,"note".into(),title,body).map(Some)
    }).await.map_err(|_|"Import interrupted")?
}
fn read_text(path:&std::path::Path,max:u64)->Result<String> {
    use std::io::Read;
    let file=std::fs::File::open(path).map_err(|_|"Could not read import")?;
    let mut bytes=Vec::new();file.take(max+1).read_to_end(&mut bytes).map_err(|_|"Could not read import")?;
    if bytes.len() as u64>max {return Err("Import exceeds the supported size".into());}
    String::from_utf8(bytes).map_err(|_|"Import must contain UTF-8 text".into())
}
fn write_export(path:&std::path::Path,bytes:&[u8])->Result<()> {
    use std::io::Write;
    let temp=path.with_file_name(format!(".hh-export-{}",ulid::Ulid::new()));
    let result=(||->Result<()>{let mut file=std::fs::OpenOptions::new().create_new(true).write(true).open(&temp).map_err(|_|"Could not save export")?;file.write_all(bytes).map_err(|_|"Could not save export")?;file.sync_all().map_err(|_|"Could not finish export")?;drop(file);std::fs::rename(&temp,path).map_err(|_|"Could not finish export. Choose a new filename.")?;Ok(())})();
    if result.is_err(){let _=std::fs::remove_file(temp);}result
}
#[tauri::command] fn shell_ready(window:WebviewWindow,state:tauri::State<'_,AppState>)->Result<()> {trusted(&window)?;state.ready.store(true,std::sync::atomic::Ordering::Release);Ok(())}
#[tauri::command] async fn finish_close(window:WebviewWindow,app:tauri::AppHandle)->Result<()> {
    trusted(&window)?;
    let state=app.state::<AppState>();
    {let saving=state.saving.lock().map_err(|_|"Downloads are busy")?;for flag in saving.values(){flag.store(true,std::sync::atomic::Ordering::Release);}}
    let started=std::time::Instant::now();
    loop {let empty=state.saving.lock().map_err(|_|"Downloads are busy")?.is_empty();if empty{break;}if started.elapsed()>std::time::Duration::from_secs(20){return Err("A drive is still finishing a file save. Wait and close Home Hub again.".into());}tokio::time::sleep(std::time::Duration::from_millis(50)).await;}
    app.exit(0);Ok(())
}
#[tauri::command] async fn browser_open(window:WebviewWindow,app:tauri::AppHandle,url:String)->Result<String> {trusted(&window)?;browser::open(&app,&url,false,vec![],false).await}
#[tauri::command] async fn youtube_open(window:WebviewWindow,app:tauri::AppHandle,url:String)->Result<String> {trusted(&window)?;let parsed=browser::public_url(&url)?;if !matches!(parsed.host_str(),Some("youtube.com"|"www.youtube.com"|"m.youtube.com")){return Err("Choose a YouTube address".into());}browser::open(&app,&url,true,vec![],false).await}
#[tauri::command] fn browser_list(window:WebviewWindow,app:tauri::AppHandle)->Result<Vec<browser::Tab>> {trusted(&window)?;let state=app.state::<AppState>();let mut tabs=state.browser.tabs.lock().map_err(|_|"Browser is busy")?;tabs.retain(|label,_|app.get_webview_window(label).is_some());Ok(tabs.values().cloned().collect())}
#[tauri::command] async fn browser_action(window:WebviewWindow,app:tauri::AppHandle,label:String,action:String,url:Option<String>)->Result<()> {
    trusted(&window)?;
    if !label.starts_with("browser-") {return Err("Invalid browser tab".into());}
    let tab=app.get_webview_window(&label).ok_or("This browser tab has closed")?;
    match action.as_str() {
        "navigate"=>tab.navigate(browser::public_url(url.as_deref().ok_or("Enter an address")?)?),
        "back"=>tab.eval("history.back()"),"forward"=>tab.eval("history.forward()"),"reload"=>tab.reload(),"focus"=>tab.set_focus(),
        "close"=>{tab.close().map_err(|_|"Could not close tab")?;app.state::<AppState>().browser.tabs.lock().map_err(|_|"Browser is busy")?.remove(&label);return Ok(());},
        _=>return Err("Unknown browser action".into())
    }.map_err(|_|"Browser action could not complete".into())
}
#[tauri::command] async fn browser_permissions(window:WebviewWindow,app:tauri::AppHandle,label:String,downloads:bool,permissions:Vec<String>)->Result<String> {
    trusted(&window)?;
    if permissions.len()>5 || permissions.iter().any(|p|!["camera","microphone","location","notifications","protected_media"].contains(&p.as_str())) {return Err("Unsupported website permission".into());}
    let state=app.state::<AppState>();
    let webview=app.get_webview_window(&label).ok_or("Tab is closed")?;
    let tab=state.browser.tabs.lock().map_err(|_|"Browser is busy")?.get(&label).cloned().ok_or("Tab is closed")?;
    // Discard the previous webview and create a fresh profile. Revocation must
    // not depend on which permissions an installed WebView2 caches persistently.
    webview.clear_all_browsing_data().map_err(|_|"Could not reset website permissions")?;
    webview.close().map_err(|_|"Could not close previous website session")?;
    state.browser.tabs.lock().map_err(|_|"Browser is busy")?.remove(&label);
    browser::open(&app,&tab.url,tab.isolated,permissions,downloads).await
}
#[tauri::command] async fn browser_clear(window:WebviewWindow,app:tauri::AppHandle)->Result<()> {
    trusted(&window)?;
    let state=app.state::<AppState>(); let labels=state.browser.tabs.lock().map_err(|_|"Browser is busy")?.keys().cloned().collect::<Vec<_>>();
    for label in labels {if let Some(tab)=app.get_webview_window(&label) {tab.clear_all_browsing_data().map_err(|_|"Could not clear website data")?;tab.close().map_err(|_|"Could not close browser")?;}}
    state.browser.tabs.lock().map_err(|_|"Browser is busy")?.clear();
    for r in state.store.list("browser","",false)? {state.store.trash(&r.id,false)?;}
    Ok(())
}
#[tauri::command] async fn open_external(window:WebviewWindow,url:String)->Result<()> {
    trusted(&window)?;let url=browser::public_url(&url)?;
    #[cfg(windows)] {std::process::Command::new(r"C:\Windows\System32\rundll32.exe").arg("url.dll,FileProtocolHandler").arg(url.as_str()).spawn().map_err(|_|"Could not open your default browser")?;Ok(())}
    #[cfg(not(windows))] {let _=url;Err("Default-browser fallback is available on Windows".into())}
}
#[tauri::command] async fn save_download(window:WebviewWindow,app:tauri::AppHandle,id:String)->Result<bool> {
    trusted(&window)?; store::validate_id(&id)?;
    let state=app.state::<AppState>();let row=state.store.list("download","",false)?.into_iter().find(|r|r.id==id).ok_or("Download no longer exists")?;
    let body:serde_json::Value=serde_json::from_str(&row.body).map_err(|_|"Invalid download")?;
    if body["status"]!="complete" {return Err("Download has not completed".into());}
    let path=PathBuf::from(body["path"].as_str().ok_or("Invalid download")?);
    let root=state.store.root.join("downloads").canonicalize().map_err(|_|"Download unavailable")?;
    let path=path.canonicalize().map_err(|_|"Download unavailable")?;
    if !path.starts_with(root) {return Err("Invalid download path".into());}
    tauri::async_runtime::spawn_blocking(move || {let Some(destination)=rfd::FileDialog::new().set_file_name(&row.title).save_file() else {return Ok(false);};std::fs::copy(path,destination).map_err(|_|"Could not save download".into()).map(|_|true)}).await.map_err(|_|"Save interrupted")?
}
#[tauri::command] async fn discard_download(window:WebviewWindow,app:tauri::AppHandle,id:String)->Result<()> {
    trusted(&window)?;store::validate_id(&id)?;
    let state=app.state::<AppState>();let row=state.store.list("download","",false)?.into_iter().find(|r|r.id==id).ok_or("Download no longer exists")?;
    let body:serde_json::Value=serde_json::from_str(&row.body).map_err(|_|"Invalid download")?;
    if body["status"]=="downloading" {return Err("Wait until this download finishes before removing it".into());}
    let path=PathBuf::from(body["path"].as_str().ok_or("Invalid download")?);
    if path.exists() {let root=state.store.root.join("downloads").canonicalize().map_err(|_|"Download unavailable")?;let canonical=path.canonicalize().map_err(|_|"Download unavailable")?;if !canonical.starts_with(root){return Err("Invalid download path".into());}tokio::fs::remove_file(canonical).await.map_err(|_|"Could not remove download")?;}
    let db=state.store.db.lock().map_err(|_|"App storage is busy")?;
    db.execute("DELETE FROM records WHERE id=?1 AND kind='download'",rusqlite::params![id]).map_err(|_|"Could not remove download")?;Ok(())
}
#[tauri::command] async fn save_hub_file(window:WebviewWindow,state:tauri::State<'_,AppState>,id:String,name:String)->Result<bool> {
    trusted(&window)?;store::validate_id(&id)?;
    let dialog_name=name.clone();
    let destination=tauri::async_runtime::spawn_blocking(move ||rfd::FileDialog::new().set_file_name(&dialog_name).save_file()).await.map_err(|_|"Save interrupted")?;
    let Some(destination)=destination else {return Ok(false);};
    let cancelled=std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    {let mut saving=state.saving.lock().map_err(|_|"Downloads are busy")?;if saving.len()>=3||saving.contains_key(&id){return Err("Wait for a current file save to finish".into());}saving.insert(id.clone(),cancelled.clone());}
    let temporary=destination.with_file_name(format!(".hh-download-{}",ulid::Ulid::new()));
    let result:Result<()> = async {
        let mut response=state.hub.request(&format!("/api/files/{id}/content"),"GET",None,None).await?;
        if !response.status().is_success() {return Err("File could not be downloaded".into());}
        let total=response.content_length();let mut done=0u64;let mut emitted=std::time::Instant::now();
        let _=window.emit("file-save-progress",serde_json::json!({"id":id,"name":name,"done":0,"total":total}));
        let mut file=tokio::fs::OpenOptions::new().create_new(true).write(true).open(&temporary).await.map_err(|_|"Could not create download")?;
        loop {
            if cancelled.load(std::sync::atomic::Ordering::Acquire) {return Err("Download cancelled".into());}
            let chunk=tokio::time::timeout(std::time::Duration::from_secs(15),response.chunk()).await.map_err(|_|"Download stalled. Try again.")?.map_err(|_|"Download was interrupted")?;
            let Some(chunk)=chunk else{break;};file.write_all(&chunk).await.map_err(|_|"Drive could not save download")?;done=done.saturating_add(chunk.len() as u64);
            if emitted.elapsed()>=std::time::Duration::from_millis(250) {let _=window.emit("file-save-progress",serde_json::json!({"id":id,"name":name,"done":done,"total":total}));emitted=std::time::Instant::now();}
        }
        if cancelled.load(std::sync::atomic::Ordering::Acquire) {return Err("Download cancelled".into());}
        if total.is_some_and(|total|total!=done){return Err("Download was incomplete".into());}
        file.sync_all().await.map_err(|_|"Could not finish saving download")?;drop(file);
        tokio::fs::rename(&temporary,&destination).await.map_err(|_|"Could not finish saving. Choose a new filename if one already exists.")?;Ok(())
    }.await;
    if result.is_err() {let _=tokio::fs::remove_file(&temporary).await;}
    if let Ok(mut saving)=state.saving.lock(){saving.remove(&id);}
    let _=window.emit("file-save-finished",serde_json::json!({"id":id,"success":result.is_ok()}));
    result.map(|_|true)
}
#[tauri::command] fn cancel_save(window:WebviewWindow,state:tauri::State<'_,AppState>,id:String)->Result<()> {trusted(&window)?;store::validate_id(&id)?;let saving=state.saving.lock().map_err(|_|"Downloads are busy")?;let cancel=saving.get(&id).ok_or("File save already finished")?;cancel.store(true,std::sync::atomic::Ordering::Release);Ok(())}
fn token_path()->PathBuf {
    let args=std::env::args().collect::<Vec<_>>();
    if let Some(i)=args.iter().position(|a|a=="--data-dir") {if let Some(dir)=args.get(i+1) {return PathBuf::from(dir).join("dashboard_token");}}
    #[cfg(windows)] {std::env::var_os("ProgramData").map(PathBuf::from).unwrap_or_else(||PathBuf::from(r"C:\ProgramData")).join("HomeHub/dashboard_token")}
    #[cfg(not(windows))] {std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(||PathBuf::from(".")).join(".homehub/dashboard_token")}
}
fn main() {
    let app=tauri::Builder::default()
        .register_asynchronous_uri_scheme_protocol("hhmedia",|context,request,responder| {
            if context.webview_label()!="main" {responder.respond(tauri::http::Response::builder().status(403).body(Vec::<u8>::new()).unwrap_or_default());return;}
            let app=context.app_handle().clone();let hub=app.state::<AppState>().hub.clone();
            tauri::async_runtime::spawn(async move {
                let path=request.uri().path();let parts=path.trim_start_matches('/').split('/').collect::<Vec<_>>();
                let result:Result<tauri::http::Response<Vec<u8>>> = async {
                    if parts.len()!=2 || store::validate_id(parts[0]).is_err() || !["content","thumb"].contains(&parts[1]) {return Err("Invalid media request".into());}
                    let range=proxy::media_range(request.headers().get("range").and_then(|v|v.to_str().ok()))?;
                    let response=hub.request(&format!("/api/files/{}/{}",parts[0],parts[1]),"GET",None,range.as_deref()).await?;
                    let origin=request.headers().get("origin").and_then(|v|v.to_str().ok()).filter(|v|matches!(*v,"http://tauri.localhost"|"https://tauri.localhost"|"tauri://localhost")||cfg!(debug_assertions)&&*v=="http://127.0.0.1:1420").unwrap_or("http://tauri.localhost");
                    let mut builder=tauri::http::Response::builder().status(response.status().as_u16()).header("Access-Control-Allow-Origin",origin).header("Cache-Control","no-store").header("X-Content-Type-Options","nosniff");
                    for name in ["content-type","content-range","accept-ranges","content-length"] {if let Some(value)=response.headers().get(name) {builder=builder.header(name,value);}}
                    let bytes=proxy::bounded_bytes(response,16*1024*1024).await?;
                    builder.body(bytes).map_err(|_|"Media response failed".into())
                }.await;
                responder.respond(result.unwrap_or_else(|_|tauri::http::Response::builder().status(502).body(Vec::new()).unwrap_or_default()));
            });
        })
        .setup(|app| {
            let root=app.path().app_local_data_dir()?;
            let state=AppState {store:Store::open(root).map_err(std::io::Error::other)?,hub:proxy::Hub::new(token_path()).map_err(std::io::Error::other)?,browser:browser::Browser::default(),ready:std::sync::atomic::AtomicBool::new(false),saving:std::sync::Mutex::new(std::collections::BTreeMap::new())};app.manage(state);
            let trusted_directory=app.path().app_local_data_dir()?.join("trusted-webview");
            store::private_directory(&trusted_directory).map_err(std::io::Error::other)?;
            let url=if cfg!(debug_assertions) {WebviewUrl::External("http://127.0.0.1:1420".parse()?)}else{WebviewUrl::App("index.html".into())};
            WebviewWindowBuilder::new(app,"main",url).title("Home Hub").inner_size(1280.0,820.0).min_inner_size(640.0,480.0).data_directory(trusted_directory)
                .on_navigation(|url|url.scheme()=="tauri"&&url.host_str()==Some("localhost")||matches!(url.scheme(),"http"|"https")&&url.host_str()==Some("tauri.localhost")||cfg!(debug_assertions)&&url.host_str()==Some("127.0.0.1")&&url.port()==Some(1420))
                .on_new_window(|_,_|tauri::webview::NewWindowResponse::Deny).build()?;
            Ok(())
        })
        .on_window_event(|window,event| {
            if window.label().starts_with("browser-") && matches!(event,tauri::WindowEvent::Destroyed) {if let Ok(mut tabs)=window.state::<AppState>().browser.tabs.lock(){tabs.remove(window.label());}}
            if window.label()=="main" {if let tauri::WindowEvent::CloseRequested {api,..}=event {
                if window.state::<AppState>().ready.load(std::sync::atomic::Ordering::Acquire) {api.prevent_close();if window.emit("shell-close-requested",()).is_err(){let _=window.set_focus();}}
                else {window.app_handle().exit(0);}
            }}
        })
        .invoke_handler(tauri::generate_handler![hub_request,local_list,local_save,local_trash,local_restore,local_purge,local_export,local_import,note_export,note_import,shell_ready,finish_close,browser_open,youtube_open,browser_action,browser_list,browser_permissions,browser_clear,open_external,save_download,discard_download,save_hub_file,cancel_save])
        .run(tauri::generate_context!());
    if let Err(error)=app {eprintln!("Home Hub could not start: {error}");}
}
