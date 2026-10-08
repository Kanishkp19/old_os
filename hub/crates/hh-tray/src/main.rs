//! Single per-owner tray. Only fixed-loopback requests; no internet update polling.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::{fs::File, io::Read, path::PathBuf, sync::{Arc, atomic::{AtomicBool, Ordering}}, time::Duration};
use clap::Parser;
use fs2::FileExt;
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use winit::{application::ApplicationHandler, event::WindowEvent, event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy}, window::WindowId};

const BASE:&str="http://127.0.0.1:47801";
const MAX_RESPONSE:u64=64*1024;
#[derive(Parser)]
#[command(name="hh-tray",about="Home Hub owner-session tray")]
struct Args {
    #[arg(long)] data_dir:Option<PathBuf>,
    #[arg(long,default_value_t=5)] poll_secs:u64,
}
#[derive(Clone,Default)]
struct Status {online:bool,name:String,free:Option<u64>,devices:u64,transfers:u64,paused:Option<bool>,language:String}
enum UserEvent {Status(Status),Menu(MenuId),Open,Message(String,String)}
#[derive(Clone)]
struct Ctx {data_dir:PathBuf,desktop:PathBuf,agent:ureq::Agent}
impl Ctx {
    fn token(&self)->Result<String,String> {
        let path=self.data_dir.join("dashboard_token");
        let meta=std::fs::symlink_metadata(&path).map_err(|_|"Home Hub service is unavailable or this Windows account is not the authorized owner.")?;
        if !meta.is_file()||meta.file_type().is_symlink()||meta.len()>512 {return Err("Invalid local service credential".into());}
        let value=std::fs::read_to_string(path).map_err(|_|"Open Home Hub as the authorized Windows owner")?;
        let value=value.trim();if value.len()<16||value.len()>256||!value.bytes().all(|b|b.is_ascii_alphanumeric()||b"-_= ".contains(&b)&&b!=b' '){return Err("Invalid local service credential".into());}Ok(value.into())
    }
    fn request(&self,path:&str,body:Option<serde_json::Value>)->Result<serde_json::Value,String> {
        if !matches!(path,"/api/overview"|"/api/settings"|"/api/pair/open"|"/api/pair/qr"|"/api/pause-sharing") {return Err("Unsupported tray action".into());}
        // Read anew for every request; the service rotates this token on restart.
        let token=self.token()?;let url=format!("{BASE}{path}");
        let response=if let Some(body)=body {self.agent.post(&url).set("X-HH-Local",&token).send_json(body)} else {self.agent.get(&url).set("X-HH-Local",&token).call()}.map_err(|_|"Home Hub is not reachable. Start the service and try again.")?;
        if response.status()==204{return Ok(serde_json::Value::Null);}
        let mut bytes=Vec::new();response.into_reader().take(MAX_RESPONSE+1).read_to_end(&mut bytes).map_err(|_|"Local response was interrupted")?;
        if bytes.len() as u64>MAX_RESPONSE{return Err("Local response exceeds tray limits".into());}
        serde_json::from_slice(&bytes).map_err(|_|"Home Hub returned an invalid response".into())
    }
    fn status(&self)->Status {
        let Ok(value)=self.request("/api/overview",None) else{return Status::default();};
        let settings=self.request("/api/settings",None).unwrap_or(serde_json::Value::Null);
        Status {online:true,name:value["name"].as_str().unwrap_or("Home Hub").chars().take(80).collect(),free:value["free_bytes"].as_u64(),devices:value["devices"].as_u64().unwrap_or(0),transfers:value["active_transfers"].as_u64().unwrap_or(0),paused:settings["pause_sharing"].as_bool(),language:settings["language"].as_str().unwrap_or("en").into()}
    }
    fn open(&self)->Result<(),String> {
        if !self.desktop.is_file(){return Err("The native Home Hub app is missing. Repair the installation.".into());}
        std::process::Command::new(&self.desktop).arg("--data-dir").arg(&self.data_dir).spawn().map_err(|_|"Could not open Home Hub")?;Ok(())
    }
    fn pair(&self)->Result<String,String> {
        self.request("/api/pair/open",Some(serde_json::json!({})))?;
        let value=self.request("/api/pair/qr",None)?;
        let code=value["manual_code"].as_str().filter(|v|v.len()==6&&v.bytes().all(|b|b.is_ascii_digit())).ok_or("Home Hub did not return a valid pairing code")?;
        let secs=value["expires_in"].as_u64().unwrap_or(0).min(300);
        self.open()?;
        Ok(format!("Scan the code in Home Hub, or enter {code} on your phone.\n\nExpires in {secs} seconds. Approve a manual pairing request in the Home Hub app."))
    }
}
fn text(language:&str,en:&str,hi:&str)->String {if language=="hi"{hi.into()}else{en.into()}}
fn gb(value:Option<u64>)->String {value.map(|n|format!("{:.1} GB",n as f64/1_073_741_824.0)).unwrap_or_else(||"—".into())}
struct App {ctx:Ctx,proxy:EventLoopProxy<UserEvent>,tray:Option<TrayIcon>,status:Status,stop:Arc<AtomicBool>,action:bool}
impl App {
    fn menu(&mut self) {
        let language=&self.status.language;
        let status=if self.status.online {format!("{} · {} · {} / {}",self.status.name,gb(self.status.free),self.status.devices,self.status.transfers)}else{text(language,"Home Hub — service unavailable","Home Hub — सेवा उपलब्ध नहीं")};
        let menu=Menu::new();
        let _=menu.append(&MenuItem::with_id("status",&status,false,None));let _=menu.append(&PredefinedMenuItem::separator());
        let _=menu.append(&MenuItem::with_id("open",text(language,"Open Home Hub","Home Hub खोलें"),true,None));
        let _=menu.append(&MenuItem::with_id("pair",text(language,"Pair a device…","डिवाइस जोड़ें…"),self.status.online&&!self.action,None));
        let pause=if self.status.paused==Some(true){text(language,"Resume sharing","साझा करना शुरू करें")}else{text(language,"Pause sharing","साझा करना रोकें")};
        let _=menu.append(&MenuItem::with_id("pause",pause,self.status.online&&self.status.paused.is_some()&&!self.action,None));
        let _=menu.append(&PredefinedMenuItem::separator());let _=menu.append(&MenuItem::with_id("quit",text(language,"Quit tray (Hub keeps running)","ट्रे बंद करें (हब चलता रहेगा)"),true,None));
        if let Some(tray)=&self.tray {let _=tray.set_menu(Some(Box::new(menu)));let _=tray.set_tooltip(Some(status));}
    }
    fn action(&mut self,event_loop:&ActiveEventLoop,id:MenuId) {
        if id==MenuId::new("quit") {self.stop.store(true,Ordering::Release);event_loop.exit();return;}
        if id==MenuId::new("open") {if let Err(e)=self.ctx.open(){dialog("Home Hub",&e);}return;}
        if self.action{return;}let ctx=self.ctx.clone();let proxy=self.proxy.clone();let paused=self.status.paused;
        if id==MenuId::new("pair")||id==MenuId::new("pause") {
            self.action=true;self.menu();
            std::thread::spawn(move|| {
                let result=if id==MenuId::new("pair"){ctx.pair()}else{ctx.request("/api/pause-sharing",Some(serde_json::json!({"paused":paused!=Some(true)}))).map(|_|String::new())};
                let message=result.unwrap_or_else(|e|e);let _=proxy.send_event(UserEvent::Message("Home Hub".into(),message));let _=proxy.send_event(UserEvent::Status(ctx.status()));
            });
        }
    }
}
fn dialog(title:&str,value:&str) {rfd::MessageDialog::new().set_title(title).set_description(value).set_buttons(rfd::MessageButtons::Ok).show();}
impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self,event_loop:&ActiveEventLoop) {
        if self.tray.is_some(){return;}
        match app_icon().and_then(|icon|TrayIconBuilder::new().with_menu(Box::new(Menu::new())).with_tooltip("Home Hub").with_icon(icon).build().map_err(|_|"Could not create the Windows tray icon".into())) {Ok(tray)=>{self.tray=Some(tray);self.menu();},Err(e)=>{dialog("Home Hub",&e);event_loop.exit();}}
    }
    fn window_event(&mut self,_:&ActiveEventLoop,_:WindowId,_:WindowEvent) {}
    fn user_event(&mut self,event_loop:&ActiveEventLoop,event:UserEvent) {
        match event {UserEvent::Status(status)=>{self.status=status;self.menu();},UserEvent::Menu(id)=>self.action(event_loop,id),UserEvent::Open=>{if let Err(e)=self.ctx.open(){dialog("Home Hub",&e);}},UserEvent::Message(title,value)=>{self.action=false;self.menu();if !value.is_empty(){dialog(&title,&value);}}}
    }
    fn exiting(&mut self,_:&ActiveEventLoop) {self.stop.store(true,Ordering::Release);self.tray=None;}
}
fn app_icon()->Result<Icon,String> {
    let mut rgba=vec![0u8;32*32*4];
    let mut put=|x:usize,y:usize,c:[u8;3]|{let i=(y*32+x)*4;rgba[i..i+3].copy_from_slice(&c);rgba[i+3]=255;};
    for y in 2..=14 {let half=(y-2)*14/12;for x in 15-half..=15+half{put(x,y,[42,108,76]);}}
    for y in 15..30{for x in 6..26{put(x,y,if (13..=18).contains(&x)&&y>=21{[30,40,35]}else{[238,241,233]});}}
    Icon::from_rgba(rgba,32,32).map_err(|_|"Could not create the Windows tray icon".into())
}
fn instance()->Result<Option<File>,String> {
    #[cfg(windows)] let root=std::env::var_os("LOCALAPPDATA").map(PathBuf::from).ok_or("Windows user profile is unavailable")?.join("HomeHub");
    #[cfg(not(windows))] let root=std::env::var_os("HOME").map(PathBuf::from).ok_or("User profile unavailable")?.join(".homehub-tray");
    std::fs::create_dir_all(&root).map_err(|_|"Cannot open tray state")?;
    let path=root.join("tray.lock");if std::fs::symlink_metadata(&path).is_ok_and(|m|m.file_type().is_symlink()){return Err("Invalid tray lock".into());}
    let file=std::fs::OpenOptions::new().create(true).truncate(false).read(true).write(true).open(path).map_err(|_|"Cannot open tray state")?;
    match file.try_lock_exclusive(){Ok(())=>Ok(Some(file)),Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>Ok(None),Err(_)=>Err("Cannot lock tray state".into())}
}
fn run()->Result<(),String> {
    let args=Args::parse();let Some(_instance)=instance()? else{return Ok(());};
    let desktop=std::env::current_exe().map_err(|_|"Cannot locate installed Home Hub")?.parent().ok_or("Cannot locate installed Home Hub")?.join("hh-desktop.exe");
    let agent=ureq::AgentBuilder::new().redirects(0).try_proxy_from_env(false).timeout_connect(Duration::from_millis(800)).timeout(Duration::from_secs(4)).build();
    let ctx=Ctx{data_dir:args.data_dir.unwrap_or_else(||hh_core::Config::default().data_dir),desktop,agent};
    let event_loop=EventLoop::<UserEvent>::with_user_event().build().map_err(|_|"Cannot start Windows tray")?;let proxy=event_loop.create_proxy();
    {let proxy=proxy.clone();MenuEvent::set_event_handler(Some(move|event:MenuEvent|{let _=proxy.send_event(UserEvent::Menu(event.id));}));}
    {let proxy=proxy.clone();TrayIconEvent::set_event_handler(Some(move|event:TrayIconEvent|{if matches!(event,TrayIconEvent::Click {button:tray_icon::MouseButton::Left,button_state:tray_icon::MouseButtonState::Up,..}){let _=proxy.send_event(UserEvent::Open);}}));}
    let stop=Arc::new(AtomicBool::new(false));let stop_poll=stop.clone();let poll_ctx=ctx.clone();let poll_proxy=proxy.clone();let interval=args.poll_secs.clamp(5,60);
    std::thread::spawn(move||{while !stop_poll.load(Ordering::Acquire){let _=poll_proxy.send_event(UserEvent::Status(poll_ctx.status()));for _ in 0..interval*4 {if stop_poll.load(Ordering::Acquire){return;}std::thread::sleep(Duration::from_millis(250));}}});
    let mut app=App{ctx,proxy,tray:None,status:Status::default(),stop:stop.clone(),action:false};let result=event_loop.run_app(&mut app).map_err(|_|"Windows tray stopped unexpectedly".into());stop.store(true,Ordering::Release);result
}
fn main(){if let Err(e)=run(){dialog("Home Hub",&e);}}
