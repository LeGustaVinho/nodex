//! Child processes started by Nodex (sidecars, builds, programs under trace, debug adapters).
//!
//! The desktop app is a GUI process: on Windows every console child would open its own console
//! window unless it is created with `CREATE_NO_WINDOW`. [`command`] applies it, and
//! [`kill_children_on_exit`] makes Windows end every descendant when Nodex exits, even when it
//! is ended abruptly, so no dotnet, node or dev server is left behind.

use std::ffi::OsStr;
use std::process::Command;

/// `CREATE_NO_WINDOW`: the child gets a console but no visible window.
#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// A [`Command`] for `program` that opens no console window on Windows.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut cmd = Command::new(program);
    hide_window(&mut cmd);
    cmd
}

/// Applies `CREATE_NO_WINDOW` to an existing command (a no-op elsewhere).
pub fn hide_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

/// Puts this process in a Windows job object that kills every process in it when the last handle
/// closes (when Nodex exits). Children inherit the job. Call it once at start-up; it does
/// nothing on other systems, where the run manager kills process groups instead.
pub fn kill_children_on_exit() {
    #[cfg(windows)]
    windows_job::install();
}

#[cfg(windows)]
mod windows_job {
    use std::sync::OnceLock;

    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JobObjectExtendedLimitInformation, SetInformationJobObject,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    /// The job handle, kept open for the life of the process (closing it would kill everything).
    struct Job(#[allow(dead_code)] HANDLE);
    // SAFETY: a job handle is a process-wide kernel handle; it is only stored, never used again.
    unsafe impl Send for Job {}
    unsafe impl Sync for Job {}

    static JOB: OnceLock<Option<Job>> = OnceLock::new();

    pub fn install() {
        JOB.get_or_init(|| {
            // SAFETY: plain Win32 calls with valid arguments; failures are reported and ignored.
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() {
                    return None;
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const _,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                if ok == 0 || AssignProcessToJobObject(job, GetCurrentProcess()) == 0 {
                    return None;
                }
                Some(Job(job))
            }
        });
    }
}
