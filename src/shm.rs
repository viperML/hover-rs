use std::{
    ffi::{CStr, CString},
    hint::black_box,
    num::NonZeroU8,
};

use libc::{sem_t, SEM_FAILED, S_IRUSR};
use nix::{
    errno::Errno,
    fcntl::OFlag,
    sys::{mman::shm_open, stat::Mode},
    Result,
};
use rand::Rng;
use tracing::{debug, instrument};

#[derive(Debug)]
pub struct ProcMutex {
    pub name: CString,
    ptr: *const sem_t,
}

impl ProcMutex {
    pub fn open<S: AsRef<CStr>>(name: S) -> Result<Self> {
        let name = name.as_ref().to_owned();

        let ptr = unsafe {
            libc::sem_open(
                name.as_ptr(),
                OFlag::empty().bits(),
                (Mode::S_IRUSR | Mode::S_IWUSR).bits(),
                1,
            )
        };

        let ptr = match ptr {
            SEM_FAILED => return Err(Errno::last()),
            x => x as *const _,
        };

        Ok(ProcMutex { name, ptr })
    }

    pub fn new() -> Result<Self> {
        let name = CString::from(
            rand::thread_rng()
                .sample_iter(rand::distributions::Alphanumeric)
                .take(10)
                .map(|b| NonZeroU8::new(b).unwrap())
                .collect::<Vec<_>>(),
        );

        debug!(?name);

        let ptr = unsafe {
            libc::sem_open(
                name.as_ptr(),
                (OFlag::O_CREAT | OFlag::O_EXCL).bits(),
                (Mode::S_IRUSR | Mode::S_IWUSR).bits(),
                1,
            )
        };

        let ptr = match ptr {
            SEM_FAILED => return Err(Errno::last()),
            x => x as *const _,
        };

        Ok(ProcMutex { name, ptr })
    }

    pub fn lock(&self) -> Result<MutexGuard> {
        let ret = unsafe { libc::sem_wait(self.ptr as _) };
        match ret {
            -1 => Err(Errno::last()),
            _ => Ok(MutexGuard { inner: self }),
        }
    }
}

// this is wrong, only the first thread should close it
impl Drop for ProcMutex {
    fn drop(&mut self) {
        let _ = unsafe { libc::sem_close(self.ptr as _) };
    }
}

#[derive(Debug)]
pub struct MutexGuard<'t> {
    inner: &'t ProcMutex,
}

impl Drop for MutexGuard<'_> {
    fn drop(&mut self) {
        let _ = unsafe { libc::sem_post(self.inner.ptr as _) };
    }
}

#[test]
fn test_sem() {
    let mutex = ProcMutex::new().unwrap();
    let name = mutex.name.clone();

    {
        mutex.lock().unwrap();

        std::thread::spawn(move || {
            let other_mutex = ProcMutex::open(name).unwrap();
            other_mutex.lock().unwrap();
        });
    }
}
