//! Bounded service logs. Diagnostics never include raw log lines.
use std::{fs::{self,File,OpenOptions},io::{self,Write},path::{Path,PathBuf},sync::{Arc,Mutex}};

pub const MAX_FILES:usize=10;
pub const MAX_FILE_BYTES:u64=5*1024*1024;

#[derive(Clone)]
pub struct BoundedLog(Arc<Mutex<State>>);
struct State{path:PathBuf,file:Option<File>,size:u64}

impl BoundedLog {
    pub fn new(dir:&Path)->io::Result<Self>{
        fs::create_dir_all(dir)?;
        let path=dir.join("hub.log");
        let file=OpenOptions::new().append(true).create(true).open(&path)?;
        let size=file.metadata()?.len();
        Ok(Self(Arc::new(Mutex::new(State{path,file:Some(file),size}))))
    }
}

impl State {
    fn rotate(&mut self)->io::Result<()> {
        self.file.take();
        let oldest=self.path.with_extension(format!("log.{}",MAX_FILES-1));
        if oldest.exists(){fs::remove_file(oldest)?;}
        for i in (1..MAX_FILES-1).rev(){
            let old=self.path.with_extension(format!("log.{i}"));
            if old.exists(){fs::rename(old,self.path.with_extension(format!("log.{}",i+1)))?;}
        }
        let first=self.path.with_extension("log.1");
        if first.exists(){fs::remove_file(&first)?;}
        fs::rename(&self.path,first)?;
        self.file=Some(OpenOptions::new().append(true).create(true).open(&self.path)?);
        self.size=0;
        Ok(())
    }
}

impl Write for BoundedLog {
    fn write(&mut self,buf:&[u8])->io::Result<usize>{
        if buf.is_empty(){return Ok(0);}
        let mut state=self.0.lock().map_err(|_|io::Error::other("log lock poisoned"))?;
        if state.size>=MAX_FILE_BYTES{state.rotate()?;}
        let n=buf.len().min((MAX_FILE_BYTES-state.size) as usize);
        state.file.as_mut().ok_or_else(||io::Error::other("log unavailable"))?.write_all(&buf[..n])?;
        state.size+=n as u64;
        Ok(n)
    }
    fn flush(&mut self)->io::Result<()> {
        let mut state=self.0.lock().map_err(|_|io::Error::other("log lock poisoned"))?;
        if let Some(file)=state.file.as_mut(){file.flush()?;}
        Ok(())
    }
}
