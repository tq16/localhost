use std::io;
use std::os::unix::io::RawFd;

pub struct Epoll {
    fd: RawFd,
}

impl Epoll {
    pub fn new() -> Result<Self, io::Error> {
        let fd = unsafe { libc::epoll_create1(0) };

        if fd < 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(Self { fd })
    }

    pub fn add(&self, fd: RawFd, events: u32) -> Result<(), io::Error> {
        self.control(libc::EPOLL_CTL_ADD, fd, events)
    }

    pub fn modify(&self, fd: RawFd, events: u32) -> Result<(), io::Error> {
        self.control(libc::EPOLL_CTL_MOD, fd, events)
    }

    pub fn delete(&self, fd: RawFd) -> Result<(), io::Error> {
        let result =
            unsafe { libc::epoll_ctl(self.fd, libc::EPOLL_CTL_DEL, fd, std::ptr::null_mut()) };

        if result < 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(())
    }

    pub fn wait(
        &self,
        events: &mut [libc::epoll_event],
        timeout_ms: i32,
    ) -> Result<usize, io::Error> {
        let result = unsafe {
            libc::epoll_wait(
                self.fd,
                events.as_mut_ptr(),
                events.len() as i32,
                timeout_ms,
            )
        };

        if result < 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(result as usize)
    }

    fn control(&self, operation: i32, fd: RawFd, events: u32) -> Result<(), io::Error> {
        let mut event = libc::epoll_event {
            events,
            u64: fd as u64,
        };

        let result = unsafe { libc::epoll_ctl(self.fd, operation, fd, &mut event) };

        if result < 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(())
    }
}

impl Drop for Epoll {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}
