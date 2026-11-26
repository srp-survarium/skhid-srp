//! Hand a connected socket from the front process to a level's zygote: one `sendmsg` with the descriptor as
//! `SCM_RIGHTS` and the bytes already read from it (the `Create` frame) as data. Unix-domain stream sockets only.

use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};

/// Largest data part (a `Create` frame: options blob + world item; the codec's frame limit).
pub const MAX_DATA: usize = world_host_proto::MAX_FRAME + 4;

/// Send `fd` with a u32 length (one `sendmsg`, the descriptor attached to those 4 bytes), then the data. The receiver
/// reads exactly 4 bytes with the descriptor and then exactly `len` bytes, so consecutive hand-overs never mix.
pub fn send_fd(sock: &impl AsRawFd, fd: RawFd, data: &[u8]) -> io::Result<()> {
    if data.len() > MAX_DATA {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "data too large"));
    }
    let head = (data.len() as u32).to_le_bytes();
    unsafe {
        let mut iov = libc::iovec { iov_base: head.as_ptr() as *mut libc::c_void, iov_len: 4 };
        let space = libc::CMSG_SPACE(std::mem::size_of::<RawFd>() as u32) as usize;
        let mut cbuf = vec![0u8; space];
        let mut msg: libc::msghdr = std::mem::zeroed();
        msg.msg_iov = &mut iov;
        msg.msg_iovlen = 1;
        msg.msg_control = cbuf.as_mut_ptr() as *mut libc::c_void;
        msg.msg_controllen = space as _;
        let c = libc::CMSG_FIRSTHDR(&msg);
        (*c).cmsg_level = libc::SOL_SOCKET;
        (*c).cmsg_type = libc::SCM_RIGHTS;
        (*c).cmsg_len = libc::CMSG_LEN(std::mem::size_of::<RawFd>() as u32) as _;
        std::ptr::write_unaligned(libc::CMSG_DATA(c) as *mut RawFd, fd);
        if libc::sendmsg(sock.as_raw_fd(), &msg, libc::MSG_NOSIGNAL) != 4 {
            return Err(io::Error::last_os_error());
        }
    }
    write_all(sock.as_raw_fd(), data)
}

fn write_all(fd: RawFd, mut b: &[u8]) -> io::Result<()> {
    while !b.is_empty() {
        let n = unsafe { libc::send(fd, b.as_ptr() as *const libc::c_void, b.len(), libc::MSG_NOSIGNAL) };
        if n < 0 {
            let e = io::Error::last_os_error();
            if e.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(e);
        }
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::WriteZero, "peer closed"));
        }
        b = &b[n as usize..];
    }
    Ok(())
}

fn read_exact(fd: RawFd, b: &mut [u8]) -> io::Result<()> {
    let mut got = 0;
    while got < b.len() {
        let n = unsafe { libc::recv(fd, b[got..].as_mut_ptr() as *mut libc::c_void, b.len() - got, 0) };
        if n < 0 {
            let e = io::Error::last_os_error();
            if e.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(e);
        }
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "data cut"));
        }
        got += n as usize;
    }
    Ok(())
}

/// Receive one descriptor with its data. `Ok(None)` = the peer closed the socket.
pub fn recv_fd(sock: &impl AsRawFd) -> io::Result<Option<(OwnedFd, Vec<u8>)>> {
    let mut head = [0u8; 4];
    let mut fd: Option<OwnedFd> = None;
    let n = unsafe {
        let mut iov = libc::iovec { iov_base: head.as_mut_ptr() as *mut libc::c_void, iov_len: 4 };
        let space = libc::CMSG_SPACE(std::mem::size_of::<RawFd>() as u32) as usize;
        let mut cbuf = vec![0u8; space];
        let mut msg: libc::msghdr = std::mem::zeroed();
        msg.msg_iov = &mut iov;
        msg.msg_iovlen = 1;
        msg.msg_control = cbuf.as_mut_ptr() as *mut libc::c_void;
        msg.msg_controllen = space as _;
        let n = loop {
            let n = libc::recvmsg(sock.as_raw_fd(), &mut msg, libc::MSG_CMSG_CLOEXEC | libc::MSG_WAITALL);
            if n < 0 && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                continue;
            }
            break n;
        };
        if n < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut c = libc::CMSG_FIRSTHDR(&msg);
        while !c.is_null() {
            if (*c).cmsg_level == libc::SOL_SOCKET && (*c).cmsg_type == libc::SCM_RIGHTS {
                fd = Some(OwnedFd::from_raw_fd(std::ptr::read_unaligned(libc::CMSG_DATA(c) as *const RawFd)));
            }
            c = libc::CMSG_NXTHDR(&msg, c);
        }
        n as usize
    };
    if n == 0 {
        return Ok(None);
    }
    if n != 4 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "short header"));
    }
    let fd = fd.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "message without a descriptor"))?;
    let len = u32::from_le_bytes(head) as usize;
    if len > MAX_DATA {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "data too large"));
    }
    let mut data = vec![0u8; len];
    read_exact(sock.as_raw_fd(), &mut data)?;
    Ok(Some((fd, data)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    #[test]
    fn a_socket_and_its_first_bytes_cross_over() {
        let (a, b) = UnixStream::pair().unwrap();
        let (mut conn_relay, conn_host) = UnixStream::pair().unwrap();
        let data: Vec<u8> = (0..200_000u32).map(|i| i as u8).collect();
        let t = std::thread::spawn(move || {
            send_fd(&a, conn_host.as_raw_fd(), &data).unwrap();
            data
        });
        let (fd, got) = recv_fd(&b).unwrap().unwrap();
        assert_eq!(got, t.join().unwrap());
        let mut moved = UnixStream::from(fd);
        conn_relay.write_all(b"ping").unwrap();
        let mut buf = [0u8; 4];
        moved.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"ping");
    }

    #[test]
    fn two_hand_overs_back_to_back_stay_apart() {
        let (a, b) = UnixStream::pair().unwrap();
        let (x1, y1) = UnixStream::pair().unwrap();
        let (x2, y2) = UnixStream::pair().unwrap();
        send_fd(&a, y1.as_raw_fd(), b"first").unwrap();
        send_fd(&a, y2.as_raw_fd(), b"second, longer").unwrap();
        let (f1, d1) = recv_fd(&b).unwrap().unwrap();
        let (f2, d2) = recv_fd(&b).unwrap().unwrap();
        assert_eq!((&d1[..], &d2[..]), (&b"first"[..], &b"second, longer"[..]));
        for (mut x, f) in [(x1, f1), (x2, f2)] {
            x.write_all(b"z").unwrap();
            let mut buf = [0u8; 1];
            UnixStream::from(f).read_exact(&mut buf).unwrap();
        }
    }

    #[test]
    fn a_closed_control_socket_is_none() {
        let (a, b) = UnixStream::pair().unwrap();
        drop(a);
        assert!(recv_fd(&b).unwrap().is_none());
    }
}
