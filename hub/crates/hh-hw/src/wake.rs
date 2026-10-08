//! Wake-on-LAN (M3, TRD §9): build and send the magic packet. Wake is
/// best-effort — the queue is the guarantee (decision D3, TEST_PLAN RM-06).

use std::net::UdpSocket;



/// Build the 102-byte magic packet: 6×0xFF + 16×MAC.
pub fn magic_packet(mac: &str) -> Result<[u8; 102]> {
    let parts: Vec<&str> = mac.split([':', '-']).collect();
    if parts.len() != 6 {
        return Err(Error::BadRequest(format!("invalid MAC {mac}")));
    }
    let mut mac_bytes = [0u8; 6];
    for (i, p) in parts.iter().enumerate() {
        mac_bytes[i] = u8::from_str_radix(p, 16)
            .map_err(|_| Error::BadRequest(format!("invalid MAC {mac}")))?;
    }
    let mut pkt = [0xFFu8; 102];
    for i in 0..16 {
        pkt[6 + i * 6..6 + (i + 1) * 6].copy_from_slice(&mac_bytes);
    }
    Ok(pkt)
}

/// Send the magic packet to the subnet broadcast address, UDP port 9.
pub fn send_wake(mac: &str, broadcast: &str) -> Result<()> {
    let pkt = magic_packet(mac)?;
    let sock = UdpSocket::bind("0.0.0.0:0")?;
    sock.set_broadcast(true)?;
    sock.send_to(&pkt, format!("{broadcast}:9"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_layout() {
        let p = magic_packet("01:23:45:67:89:ab").unwrap();
        assert_eq!(&p[..6], &[0xFF; 6]);
        assert_eq!(&p[6..12], &[0x01, 0x23, 0x45, 0x67, 0x89, 0xab]);
        assert_eq!(&p[96..102], &[0x01, 0x23, 0x45, 0x67, 0x89, 0xab]);
        assert!(magic_packet("bad").is_err());
    }
}

/// Opt-in Task Scheduler wake request. Firmware/power policy may decline it.
use hh_core::{Error,Result};
use std::path::Path;
pub fn validate_time(time:&str)->Result<()> {
    let valid=time.is_ascii()&&time.len()==5&&time.as_bytes()[2]==b':'&&time[..2].parse::<u8>().is_ok_and(|v|v<24)&&time[3..].parse::<u8>().is_ok_and(|v|v<60);
    if !valid {return Err(Error::BadRequest("wake time must be HH:MM".into()));}Ok(())
}
pub fn configure(enabled:bool,time:&str,executable:&Path)->Result<()> {
    validate_time(time)?;
    #[cfg(not(windows))] {let _=executable;if enabled{return Err(Error::BadRequest("scheduled wake requires Windows".into()));}Ok(())}
    #[cfg(windows)] {
        let mut command=std::process::Command::new("schtasks.exe");
        let temporary=std::env::temp_dir().join(format!("homehub-wake-{}.xml",ulid::Ulid::new()));
        if enabled {
            let exe=executable.to_string_lossy().replace('&',"&amp;").replace('<',"&lt;").replace('>',"&gt;").replace('"',"&quot;");
            let xml=format!(r#"<?xml version="1.0" encoding="UTF-8"?><Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task"><Triggers><CalendarTrigger><StartBoundary>2020-01-01T{time}:00</StartBoundary><Enabled>true</Enabled><ScheduleByDay><DaysInterval>1</DaysInterval></ScheduleByDay></CalendarTrigger></Triggers><Principals><Principal id="System"><UserId>S-1-5-18</UserId><RunLevel>HighestAvailable</RunLevel></Principal></Principals><Settings><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy><DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries><StopIfGoingOnBatteries>false</StopIfGoingOnBatteries><WakeToRun>true</WakeToRun><ExecutionTimeLimit>PT1M</ExecutionTimeLimit></Settings><Actions Context="System"><Exec><Command>{exe}</Command><Arguments>--wake-only</Arguments></Exec></Actions></Task>"#);
            let mut file=std::fs::OpenOptions::new().create_new(true).write(true).open(&temporary)?;
            use std::io::Write;file.write_all(xml.as_bytes())?;file.sync_all()?;
            command.args(["/Create","/TN","HomeHub Scheduled Wake","/F","/XML"]).arg(&temporary);
        }else{command.args(["/Delete","/TN","HomeHub Scheduled Wake","/F"]);}
        use std::os::windows::process::CommandExt;command.creation_flags(0x08000000).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        let mut child=command.spawn()?;let deadline=std::time::Instant::now()+std::time::Duration::from_secs(15);
        let result=loop {if let Some(status)=child.try_wait()?{break if status.success()||!enabled{Ok(())}else{Err(Error::Internal("Windows rejected the scheduled wake request".into()))};}if std::time::Instant::now()>=deadline{let _=child.kill();let _=child.wait();break Err(Error::Internal("Task Scheduler timed out".into()));}std::thread::sleep(std::time::Duration::from_millis(50));};
        let _=std::fs::remove_file(temporary);result
    }
}
# [cfg(test)]mod schedule_tests {use super::*;#[test]fn times(){for time in ["00:00","23:59"]{assert!(validate_time(time).is_ok());}for time in ["24:00","12:60","1:00","é:00"]{assert!(validate_time(time).is_err());}}}
