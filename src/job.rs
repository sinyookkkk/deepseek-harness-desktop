//! Windows Job Object ownership of the sidecar process tree.

use std::io;
use std::mem::size_of;
use std::os::windows::io::AsRawHandle;
use std::process::Child;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject,
};

/// A kill-on-close Job Object that owns every descendant of the sidecar.
pub struct Job(HANDLE);

// The handle is an owned kernel object; all operations are thread-safe.
unsafe impl Send for Job {}
unsafe impl Sync for Job {}

impl Job {
    /// Create a job whose close terminates every member.
    pub fn create() -> io::Result<Self> {
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let configured = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION as *const core::ffi::c_void,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configured == 0 {
            let error = io::Error::last_os_error();
            unsafe { CloseHandle(handle) };
            return Err(error);
        }
        Ok(Self(handle))
    }

    /// Put a spawned child into this job.
    ///
    /// shortcut: the child is assigned immediately after spawn, so a
    /// descendant created in that window escapes the job. dsh starts tool
    /// processes only after the Web UI is ready. Upgrade with
    /// CREATE_SUSPENDED, assignment, and thread resume if early descendants
    /// become possible.
    ///
    /// @param child - the just-spawned sidecar.
    pub fn assign(&self, child: &Child) -> io::Result<()> {
        let assigned = unsafe { AssignProcessToJobObject(self.0, child.as_raw_handle() as HANDLE) };
        if assigned == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// Terminate every process in the job.
    pub fn terminate(&self) {
        unsafe { TerminateJobObject(self.0, 1) };
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        self.terminate();
        unsafe { CloseHandle(self.0) };
    }
}
