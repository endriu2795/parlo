use crate::LockExt;
use std::process::Child;
use std::sync::Mutex;

// A venv launcher can create another Python process. Own the complete job so
// cancelling an obsolete preview releases the actual inference process too.
pub struct EngineChild {
    child: Mutex<Child>,
    #[cfg(windows)]
    job: isize,
}
impl EngineChild {
    pub fn new(mut child: Child) -> Result<Self, String> {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows::Win32::{
                Foundation::{CloseHandle, HANDLE},
                System::JobObjects::*,
            };
            let result = unsafe {
                CreateJobObjectW(None, None).and_then(|job| {
                    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
                    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                    let attached = SetInformationJobObject(
                        job,
                        JobObjectExtendedLimitInformation,
                        &limits as *const _ as _,
                        std::mem::size_of_val(&limits) as u32,
                    )
                    .and_then(|_| AssignProcessToJobObject(job, HANDLE(child.as_raw_handle())));
                    match attached {
                        Ok(()) => Ok(job.0 as isize),
                        Err(error) => {
                            let _ = CloseHandle(job);
                            Err(error)
                        }
                    }
                })
            };
            match result {
                Ok(job) => {
                    return Ok(Self {
                        child: Mutex::new(child),
                        job,
                    })
                }
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(error.to_string());
                }
            }
        }
        #[cfg(not(windows))]
        Ok(Self {
            child: Mutex::new(child),
        })
    }
    pub fn exited(&self) -> bool {
        self.child
            .guard()
            .try_wait()
            .ok()
            .flatten()
            .is_some()
    }
    pub fn success(&self) -> bool {
        self.child.guard().wait().map(|status| status.success()).unwrap_or(false)
    }
    pub fn exit_code(&self) -> Option<i32> {
        self.child.guard().try_wait().ok().flatten().and_then(|status| status.code())
    }
    pub fn cancel(&self) {
        #[cfg(windows)]
        unsafe {
            let _ = windows::Win32::System::JobObjects::TerminateJobObject(
                windows::Win32::Foundation::HANDLE(self.job as _),
                1,
            );
        }
        #[cfg(unix)] {
            let mut child = self.child.guard();
            if child.try_wait().ok().flatten().is_none() {
                // Commands owned by this supervisor start in their own group.
                // Kill the inference CLI descendants along with Python.
                unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL); }
                let _ = child.kill();
            }
        }
    }
}
impl Drop for EngineChild {
    fn drop(&mut self) {
        self.cancel();
        let _ = self.child.guard().wait();
        #[cfg(windows)]
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(windows::Win32::Foundation::HANDLE(
                self.job as _,
            ));
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    use windows::Win32::{
        Foundation::{CloseHandle, WAIT_OBJECT_0},
        System::Threading::*,
    };
    #[test]
    fn cancelling_preview_terminates_its_descendants() {
        // Two synthetic sleeping processes model a venv launcher and its child.
        let mut child = Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", "$p=Start-Process -FilePath powershell.exe -ArgumentList '-NoProfile','-Command','Start-Sleep -Seconds 30' -WindowStyle Hidden -PassThru; [Console]::WriteLine($p.Id); Start-Sleep -Seconds 30"])
            .creation_flags(0x08000000).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null())
            .spawn().unwrap();
        let output = child.stdout.take().unwrap();
        let supervisor = EngineChild::new(child).unwrap();
        let mut line = String::new();
        BufReader::new(output).read_line(&mut line).unwrap();
        let pid = line.trim().parse::<u32>().unwrap();
        unsafe {
            let handle = OpenProcess(PROCESS_SYNCHRONIZE, false, pid).unwrap();
            supervisor.cancel();
            let result = WaitForSingleObject(handle, 5000);
            let _ = CloseHandle(handle);
            assert_eq!(
                result, WAIT_OBJECT_0,
                "The preview's Python descendant must not survive cancellation"
            );
        }
    }
}
