//! Prevent idle sleep while the final-processing queue is active. Manual sleep
//! and closing a laptop lid remain under the user's control.
pub struct ProcessingPower {
    #[cfg(windows)]
    previous: u32,
    #[cfg(target_os = "macos")]
    child: Option<std::process::Child>,
    // Windows execution state belongs to the current thread.
    _same_thread: std::marker::PhantomData<std::rc::Rc<()>>,
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn SetThreadExecutionState(flags: u32) -> u32;
}

impl ProcessingPower {
    pub fn new() -> Self {
        Self {
            #[cfg(windows)]
            previous: unsafe { SetThreadExecutionState(0x80000001) },
            #[cfg(target_os = "macos")]
            child: std::process::Command::new("/usr/bin/caffeinate")
                .args(["-i", "-w", &std::process::id().to_string()])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .ok(),
            _same_thread: std::marker::PhantomData,
        }
    }
}
impl Drop for ProcessingPower {
    fn drop(&mut self) {
        #[cfg(windows)]
        if self.previous != 0 {
            unsafe {
                SetThreadExecutionState(self.previous | 0x80000000);
            }
        }
        #[cfg(target_os = "macos")]
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
