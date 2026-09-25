//! Passing open files between processes over a Unix socket (`SCM_RIGHTS`).
//!
//! macOS' `authopen` hands MoonDisk the raw device it opened this way, and
//! the native macOS app hands the privileged helper the disk image the user
//! picked — a file the helper, running as root, may not be allowed to open
//! itself (macOS privacy protection covers Downloads, Documents, …).

use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::net::UnixStream;

/// Room for a few descriptors per message, aligned for `cmsghdr`.
type Control = [u64; 16];

/// Reads bytes into `buf` like `read`, plus any file descriptors sent
/// along with them.
pub fn recv(socket: &UnixStream, buf: &mut [u8]) -> io::Result<(usize, Vec<OwnedFd>)> {
    let mut iov = libc::iovec {
        iov_base: buf.as_mut_ptr().cast(),
        iov_len: buf.len(),
    };
    let mut control: Control = [0; 16];
    // SAFETY: an all-zero `msghdr` is valid; the pointers set below outlive
    // the `recvmsg` call.
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = control.as_mut_ptr().cast();
    msg.msg_controllen = std::mem::size_of_val(&control) as _;

    let received = loop {
        // SAFETY: `msg` points to valid, writable buffers of the given sizes.
        let n = unsafe { libc::recvmsg(socket.as_raw_fd(), &mut msg, 0) };
        if n >= 0 {
            break n as usize;
        }
        let err = io::Error::last_os_error();
        if err.kind() != io::ErrorKind::Interrupted {
            return Err(err);
        }
    };

    let mut fds = Vec::new();
    // SAFETY: `recvmsg` filled `msg`; the CMSG macros stay within
    // `msg_controllen` bytes of `control`.
    unsafe {
        let mut cmsg = libc::CMSG_FIRSTHDR(&msg);
        while !cmsg.is_null() {
            if (*cmsg).cmsg_level == libc::SOL_SOCKET && (*cmsg).cmsg_type == libc::SCM_RIGHTS {
                let data = libc::CMSG_DATA(cmsg);
                let header = data as usize - cmsg as usize;
                let count =
                    ((*cmsg).cmsg_len as usize - header) / std::mem::size_of::<libc::c_int>();
                for i in 0..count {
                    let fd = std::ptr::read_unaligned(data.cast::<libc::c_int>().add(i));
                    fds.push(OwnedFd::from_raw_fd(fd));
                }
            }
            cmsg = libc::CMSG_NXTHDR(&msg, cmsg);
        }
    }
    Ok((received, fds))
}

/// Sends `data` (at least one byte) with `fd` attached.
pub fn send(socket: &UnixStream, data: &[u8], fd: &impl AsRawFd) -> io::Result<()> {
    assert!(
        !data.is_empty(),
        "a descriptor must travel with at least one byte"
    );
    let mut iov = libc::iovec {
        iov_base: data.as_ptr() as *mut libc::c_void,
        iov_len: data.len(),
    };
    let mut control: Control = [0; 16];
    // SAFETY: as in `recv`; the control buffer is large enough for one fd.
    unsafe {
        let space = libc::CMSG_SPACE(std::mem::size_of::<libc::c_int>() as _);
        let mut msg: libc::msghdr = std::mem::zeroed();
        msg.msg_iov = &mut iov;
        msg.msg_iovlen = 1;
        msg.msg_control = control.as_mut_ptr().cast();
        msg.msg_controllen = space as _;
        let cmsg = libc::CMSG_FIRSTHDR(&msg);
        (*cmsg).cmsg_level = libc::SOL_SOCKET;
        (*cmsg).cmsg_type = libc::SCM_RIGHTS;
        (*cmsg).cmsg_len = libc::CMSG_LEN(std::mem::size_of::<libc::c_int>() as _) as _;
        std::ptr::write_unaligned(libc::CMSG_DATA(cmsg).cast::<libc::c_int>(), fd.as_raw_fd());
        if libc::sendmsg(socket.as_raw_fd(), &msg, 0) < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Seek, Write};

    fn temp_file(content: &[u8]) -> std::fs::File {
        let path = std::env::temp_dir().join(format!("moondisk-fdpass-{}", std::process::id()));
        let mut f = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        std::fs::remove_file(&path).unwrap();
        f.write_all(content).unwrap();
        f.rewind().unwrap();
        f
    }

    #[test]
    fn passes_an_open_file_along_with_data() {
        let (a, b) = UnixStream::pair().unwrap();
        let file = temp_file(b"moon");
        send(&a, b"\n", &file).unwrap();
        drop(file);

        let mut buf = [0u8; 16];
        let (n, fds) = recv(&b, &mut buf).unwrap();
        assert_eq!(&buf[..n], b"\n");
        assert_eq!(fds.len(), 1);
        let mut received = std::fs::File::from(fds.into_iter().next().unwrap());
        let mut text = String::new();
        received.read_to_string(&mut text).unwrap();
        assert_eq!(text, "moon");
    }

    #[test]
    fn plain_data_carries_no_descriptors() {
        let (mut a, b) = UnixStream::pair().unwrap();
        a.write_all(b"hello").unwrap();
        let mut buf = [0u8; 16];
        let (n, fds) = recv(&b, &mut buf).unwrap();
        assert_eq!(&buf[..n], b"hello");
        assert!(fds.is_empty());
    }
}
