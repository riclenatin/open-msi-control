//! Start with Windows via a Task Scheduler logon task.
//!
//! The app needs Administrator rights, and Windows does not launch elevated programs from the
//! `Run` registry key. A scheduled task with "Run with highest privileges" starts it elevated at
//! logon without a UAC prompt. The task XML lets it run on battery and without a time limit,
//! which `schtasks /Create` flags cannot express.

use std::io;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Output};

pub const TASK_NAME: &str = "open-msi-control";
/// Command-line flag for starting hidden in the tray.
pub const TRAY_FLAG: &str = "--tray";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn schtasks(args: &[&str]) -> io::Result<Output> {
    Command::new("schtasks.exe").args(args).creation_flags(CREATE_NO_WINDOW).output()
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Task definition: at `user`'s logon (after 5 s) run `exe --tray` with highest privileges.
pub fn task_xml(exe: &Path, user: &str) -> String {
    let exe = xml_escape(&exe.display().to_string());
    let user = xml_escape(user);
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Description>Open MSI Control: start in the tray at logon</Description></RegistrationInfo>
  <Triggers><LogonTrigger><Enabled>true</Enabled><UserId>{user}</UserId><Delay>PT5S</Delay></LogonTrigger></Triggers>
  <Principals><Principal id="Author"><UserId>{user}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>HighestAvailable</RunLevel></Principal></Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Priority>5</Priority>
  </Settings>
  <Actions Context="Author"><Exec><Command>{exe}</Command><Arguments>{TRAY_FLAG}</Arguments></Exec></Actions>
</Task>
"#
    )
}

pub fn is_enabled() -> bool {
    schtasks(&["/Query", "/TN", TASK_NAME]).is_ok_and(|o| o.status.success())
}

/// Creates or removes the task. Requires an elevated process (the app always is).
pub fn set_enabled(enabled: bool) -> io::Result<()> {
    let output = if enabled {
        let exe = std::env::current_exe()?;
        let user = format!(
            "{}\\{}",
            std::env::var("USERDOMAIN").unwrap_or_default(),
            std::env::var("USERNAME").unwrap_or_default()
        );
        // schtasks wants UTF-16 LE with BOM to match the declared encoding.
        let xml: Vec<u8> = std::iter::once(0xFEFF_u16)
            .chain(task_xml(&exe, &user).encode_utf16())
            .flat_map(u16::to_le_bytes)
            .collect();
        let path = std::env::temp_dir().join("open-msi-control-task.xml");
        std::fs::write(&path, xml)?;
        let path_str = path.display().to_string();
        let out = schtasks(&["/Create", "/TN", TASK_NAME, "/XML", &path_str, "/F"]);
        let _ = std::fs::remove_file(&path);
        out?
    } else {
        schtasks(&["/Delete", "/TN", TASK_NAME, "/F"])?
    };
    if output.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(String::from_utf8_lossy(&output.stderr).trim().to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_xml_escapes_paths_and_runs_elevated_on_battery() {
        let xml = task_xml(Path::new(r"C:\Apps\A&B <x>\open-msi-control.exe"), r"PC\me");
        assert!(xml.contains(r"<Command>C:\Apps\A&amp;B &lt;x&gt;\open-msi-control.exe</Command>"));
        assert!(xml.contains("<RunLevel>HighestAvailable</RunLevel>"));
        assert!(xml.contains("<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>"));
        assert!(xml.contains("<Arguments>--tray</Arguments>"));
        assert!(xml.contains(r"<UserId>PC\me</UserId>"));
    }
}
