//! Fixed loopback transport. No caller can supply a host, token, or redirect destination.
use base64::Engine;
use serde::Serialize;
use std::path::PathBuf;
use crate::store::Result;
const MAX_RESPONSE: usize = 16*1024*1024;
#[derive(Clone)] pub struct Hub { pub client: reqwest::Client, pub token_path:PathBuf }
#[derive(Serialize)] pub struct Reply { pub status:u16, pub body:String, pub content_type:String, pub qr_svg:Option<String> }
impl Hub {
    pub fn new(token_path:PathBuf)->Result<Self> {
        let client=reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none()).timeout(std::time::Duration::from_secs(60)).build().map_err(|_|"Could not initialize local connection")?;
        Ok(Self {client,token_path})
    }
    pub fn token(&self)->Result<String> {
        let meta=std::fs::symlink_metadata(&self.token_path).map_err(|_|"Home Hub service is unavailable. Start it and try again.")?;
        if !meta.is_file() || meta.file_type().is_symlink() || meta.len()>512 {return Err("Invalid local service credential".into());}
        let value=std::fs::read_to_string(&self.token_path).map_err(|_|"Cannot access Home Hub. Open the app as the installed owner.")?;
        let value=value.trim();
        if value.is_empty() || value.len()>256 || !value.bytes().all(|c|c.is_ascii_alphanumeric()||b"-_=".contains(&c)) {return Err("Invalid local service credential".into());}
        Ok(value.into())
    }
    pub async fn request(&self,path:&str,method:&str,body:Option<String>,range:Option<&str>)->Result<reqwest::Response> {
        validate_path(path)?;
        let method=match method {"GET"=>reqwest::Method::GET,"POST"=>reqwest::Method::POST,"PATCH"=>reqwest::Method::PATCH,"DELETE"=>reqwest::Method::DELETE,_=>return Err("Unsupported action".into())};
        let mut request=self.client.request(method,format!("http://127.0.0.1:47801{path}")).header("X-HH-Local",self.token()?);
        if let Some(body)=body {if body.len()>1024*1024 {return Err("Request is too large".into());} serde_json::from_str::<serde_json::Value>(&body).map_err(|_|"Invalid request data")?; request=request.header("Content-Type","application/json").body(body);}
        if let Some(range)=range {if range.len()>80 {return Err("Invalid byte range".into());} request=request.header("Range",range);}
        request.send().await.map_err(|_|"Home Hub is not reachable. Start the service and try again.".into())
    }
    pub async fn reply(&self,path:&str,method:&str,body:Option<String>)->Result<Reply> {
        let response=self.request(path,method,body,None).await?;
        let status=response.status().as_u16();
        let content_type=response.headers().get("content-type").and_then(|v|v.to_str().ok()).unwrap_or("application/octet-stream").to_owned();
        let qr_svg=response.headers().get("x-qr-svg").and_then(|v|v.to_str().ok()).map(str::to_owned);
        let bytes=bounded_bytes(response,MAX_RESPONSE).await?;
        Ok(Reply {status,body:base64::engine::general_purpose::STANDARD.encode(bytes),content_type,qr_svg})
    }
}
pub async fn bounded_bytes(mut response:reqwest::Response,max:usize)->Result<Vec<u8>> {
    if response.content_length().is_some_and(|v|v>max as u64) {return Err("Item is too large for this preview. Save it instead.".into());}
    let mut bytes=Vec::new();
    while let Some(chunk)=response.chunk().await.map_err(|_|"Local connection was interrupted")? {if bytes.len()+chunk.len()>max {return Err("Item is too large for this preview. Save it instead.".into());} bytes.extend_from_slice(&chunk);}
    Ok(bytes)
}
pub fn validate_path(path:&str)->Result<()> {
    if !path.starts_with("/api/") || path.len()>4096 || path.contains(['\\','\r','\n','#']) || path.contains("..") || path.contains('%') && path.to_ascii_lowercase().contains("%2f") {return Err("Invalid local request".into());}
    let parsed=url::Url::parse(&format!("http://127.0.0.1:47801{path}")).map_err(|_|"Invalid local request")?;
    if parsed.host_str()!=Some("127.0.0.1") || parsed.port()!=Some(47801) || !parsed.path().starts_with("/api/") {return Err("Invalid local request".into());}
    Ok(())
}
#[cfg(test)] mod tests {use super::*; #[test] fn fixed_host_only(){ for p in ["https://evil.test/","//evil.test/api/","/api/../admin","/api/x\\evil","/api/x%2fsecret"] {assert!(validate_path(p).is_err(),"{p}");} assert!(validate_path("/api/files?q=holiday&limit=100").is_ok());}}
