//! Own only a process group we created; never kill by executable name or a stale stored PID.
use std::ops::{Deref, DerefMut};
use tokio::process::{Child, Command};

pub struct OwnedChild(Child);
impl OwnedChild {
    pub fn spawn(mut command: Command) -> std::io::Result<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.as_std_mut().process_group(0);
        }
        command.kill_on_drop(true).spawn().map(Self)
    }
}
impl Deref for OwnedChild {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.0
    }
}
impl DerefMut for OwnedChild {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.0
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(pid) = self
            .0
            .id()
            .and_then(|id| i32::try_from(id).ok())
            .filter(|pid| *pid > 0)
        {
            // A waited/reaped child has id=None. Validate the isolated group while its leader is still owned.
            // Descendants that explicitly escape the group and remote execution remain unverified.
            unsafe {
                if libc::getpgid(pid) == pid {
                    libc::kill(-pid, libc::SIGKILL);
                }
            }
        }
        let _ = self.0.start_kill();
    }
}
