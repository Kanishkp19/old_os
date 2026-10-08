//! Bounded logs: ten files of at most 5 MiB, no unbounded daily archives.
use std::{io::{self,Write},path::{Path,PathBuf},sync::{Arc,Mutex}};
const LIMIT:u64=5*1024*1024;
#[derive(Clone)]pub struct BoundedLog(Arc<Mutex<State>>);
struct State{path:PathBuf,file:Option<std::fs::File>,size:u64}
impl BoundedLog {
    pub fn new(dir:&Path)->Self{let path=dir.join("hub.log");let file=std::fs::OpenOptions::new().append(true).create(true).open(&path).ok();let size=file.as_ref().and_then(|f|f.metadata().ok()).map(|m|m.len()).unwrap_or(0);Self(Arc::new(Mutex::new(State{path,file,size})))}
}
impl Write for BoundedLog {
    fn write(&mut self,buf:&[u8])->io::Result<usize>{let mut s=self.0.lock().map_err(|_|io::Error::other("log lock poisoned"))?;
        let n=buf.len().min(LIMIT as usize);
        if s.size+n as u64>LIMIT {s.file.take();
            let oldest=s.path.with_extension("log.9");if oldest.exists(){std::fs::remove_file(oldest)?;}
            for i in (1..9).rev(){let old=s.path.with_extension(format!("log.{i}"));if old.exists(){std::fs::rename(old,s.path.with_extension(format!("log.{}",i+1)))?;}}
            if s.path.exists(){let first=s.path.with_extension("log.1");if first.exists(){std::fs::remove_file(&first)?;}std::fs::rename(&s.path,first)?;}
            s.file=Some(std::fs::OpenOptions::new().append(true).create(true).open(&s.path)?);s.size=0;
        }
        let file=s.file.as_mut().ok_or_else(||io::Error::other("log unavailable"))?;file.write_all(&buf[..n])?;s.size+=n as u64;Ok(n)
    }
    fn flush(&mut self)->io::Result<()> {let mut s=self.0.lock().map_err(|_|io::Error::other("log lock poisoned"))?;if let Some(f)=s.file.as_mut(){f.flush()?;}Ok(())}
}
