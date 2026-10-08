#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod browser;
mod proxy;
mod store;
use tauri::{Manager, WebviewWindow, WebviewWindowBuilder, WebviewUrl};
use store::{Result, Record, Store};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

pub struct AppState {store:Store, hub:proxy::Hub, browser:browser::Browser}
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
#[tauri::command] async fn local_export(window:WebviewWindow,state:tauri::State<'_,AppState>)->Result<bool> {
    trusted(&window)?; let text=state.store.export()?;
    tauri::async_runtime::spawn_blocking(move || {let Some(path)=rfd::FileDialog::new().set_file_name("home-hub-private.json").add_filter("Home Hub app data",&["json"]).save_file() else {return Ok(false);}; std::fs::write(path,text).map_err(|_|"Could not save export".into()).map(|_|true)}).await.map_err(|_|"Export was interrupted")?
}
#[tauri::command] async fn local_import(window:WebviewWindow,app:tauri::AppHandle)->Result<Option<usize>> {
    trusted(&window)?;
    tauri::async_runtime::spawn_blocking(move || {let Some(path)=rfd::FileDialog::new().add_filter("Home Hub app data",&["json"]).pick_file() else {return Ok(None);};if std::fs::metadata(&path).map_err(|_|"Could not read export")?.len()>8*1024*1024 {return Err("Import exceeds 8 MB".into());} let text=std::fs::read_to_string(path).map_err(|_|"Could not read export")?;app.state::<AppState>().store.import(&text).map(Some)}).await.map_err(|_|"Import was interrupted")?
}
#[tauri::command] async fn browser_open(window:WebviewWindow,app:tauri::AppHandle,url:String)->Result<String> {trusted(&window)?;browser::open(&app,&url).await}
#[tauri::command] fn browser_list(window:WebviewWindow,state:tauri::State<'_,AppState>)->Result<Vec<browser::Tab>> {trusted(&window)?; Ok(state.browser.tabs.lock().map_err(|_|"Browser is busy")?.values().cloned().collect())}
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
#[tauri::command] fn browser_permissions(window:WebviewWindow,state:tauri::State<'_,AppState>,label:String,downloads:bool,permissions:Vec<String>)->Result<()> {
    trusted(&window)?;
    if permissions.len()>5 || permissions.iter().any(|p|!["camera","microphone","location","notifications","protected_media"].contains(&p.as_str())) {return Err("Unsupported website permission".into());}
    let mut tabs=state.browser.tabs.lock().map_err(|_|"Browser is busy")?;let tab=tabs.get_mut(&label).ok_or("Tab is closed")?;
    let origin=browser::public_url(&tab.url)?.origin().ascii_serialization();
    tab.downloads=downloads; tab.permissions=permissions.iter().map(|p|format!("{origin}|{p}")).collect();Ok(())
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
#[tauri::command] async fn save_hub_file(window:WebviewWindow,state:tauri::State<'_,AppState>,id:String,name:String)->Result<bool> {
    trusted(&window)?;store::validate_id(&id)?;
    let destination=tauri::async_runtime::spawn_blocking(move ||rfd::FileDialog::new().set_file_name(&name).save_file()).await.map_err(|_|"Save interrupted")?;
    let Some(destination)=destination else {return Ok(false);};
    let temporary=destination.with_file_name(format!(".hh-download-{}",ulid::Ulid::new()));
    let mut response=state.hub.request(&format!("/api/files/{id}/content"),"GET",None,None).await?;
    if !response.status().is_success() {return Err("File could not be downloaded".into());}
    let result:Result<()> = async {
        let mut file=tokio::fs::OpenOptions::new().create_new(true).write(true).open(&temporary).await.map_err(|_|"Could not create download")?;
        while let Some(chunk)=response.chunk().await.map_err(|_|"Download was interrupted")? {file.write_all(&chunk).await.map_err(|_|"Drive could not save download")?;}
        file.sync_all().await.map_err(|_|"Could not finish saving download")?;drop(file);
        tokio::fs::rename(&temporary,&destination).await.map_err(|_|"Could not finish saving. Choose a new filename if one already exists.")?;Ok(())
    }.await;
    if result.is_err() {let _=tokio::fs::remove_file(&temporary).await;}
    result.map(|_|true)
}
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
                    let range=request.headers().get("range").and_then(|v|v.to_str().ok());
                    let default_range=if parts[1]=="content"&&range.is_none(){Some("bytes=0-1048575")}else{range};
                    let response=hub.request(&format!("/api/files/{}/{}",parts[0],parts[1]),"GET",None,default_range).await?;
                    let mut builder=tauri::http::Response::builder().status(response.status().as_u16()).header("Access-Control-Allow-Origin","http://tauri.localhost");
                    for name in ["content-type","content-range","accept-ranges","content-length"] {if let Some(value)=response.headers().get(name) {builder=builder.header(name,value);}}
                    let bytes=proxy::bounded_bytes(response,16*1024*1024).await?;
                    builder.body(bytes).map_err(|_|"Media response failed".into())
                }.await;
                responder.respond(result.unwrap_or_else(|_|tauri::http::Response::builder().status(502).body(Vec::new()).unwrap_or_default()));
            });
        })
        .setup(|app| {
            let root=app.path().app_local_data_dir()?;
            let state=AppState {store:Store::open(root).map_err(std::io::Error::other)?,hub:proxy::Hub::new(token_path()).map_err(std::io::Error::other)?,browser:browser::Browser::default()};app.manage(state);
            let url=if cfg!(debug_assertions) {WebviewUrl::External("http://127.0.0.1:1420".parse()?)}else{WebviewUrl::App("index.html".into())};
            WebviewWindowBuilder::new(app,"main",url).title("Home Hub").inner_size(1280.0,820.0).min_inner_size(640.0,480.0).data_directory(app.path().app_local_data_dir()?.join("trusted-webview"))
                .on_navigation(|url|url.scheme()=="tauri"&&url.host_str()==Some("localhost")||matches!(url.scheme(),"http"|"https")&&url.host_str()==Some("tauri.localhost")||cfg!(debug_assertions)&&url.host_str()==Some("127.0.0.1")&&url.port()==Some(1420))
                .on_new_window(|_,_|tauri::webview::NewWindowResponse::Deny).build()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![hub_request,local_list,local_save,local_trash,local_restore,local_export,local_import,browser_open,browser_action,browser_list,browser_permissions,browser_clear,open_external,save_download,save_hub_file])
        .run(tauri::generate_context!());
    if let Err(error)=app {eprintln!("Home Hub could not start: {error}");}
}
