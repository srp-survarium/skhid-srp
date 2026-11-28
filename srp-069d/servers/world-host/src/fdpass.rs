//! Hand a connected socket from the front process to a level's zygote: one `sendmsg` with the descriptor as
//! `SCM_RIGHTS` and the bytes already read from it (the `Create` frame) as data. Unix-domain stream sockets only.

use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};

/// Largest data part (a `Create` frame: options blob + world item; the codec's frame limit).
pub const MAX_DATA: usize = world_host_proto::MAX_FRAME + 4;

/// Send `fd` with a u32 length (one `sendmsg`, the descriptor attached to those 4 bytes), then the data. The receiver
/// reads exactly 4 bytes with the descriptor and then exactly `len` bytes, so consecutive hand-overs never mix.
pub fn send_fd(sock: &impl AsRawFd, fd: RawFd, data: &[u8]) -> io::Result<()> {
    send_msg(sock, Some(fd), data)
}

/// [`send_fd`] with the descriptor optional (the base process's replies: a pidfd, or none with a failure).
pub fn send_msg(sock: &impl AsRawFd, fd: Option<RawFd>, data: &[u8]) -> io::Result<()> {
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
        if let Some(fd) = fd {
            msg.msg_control = cbuf.as_mut_ptr() as *mut libc::c_void;
            msg.msg_controllen = space as _;
            let c = libc::CMSG_FIRSTHDR(&msg);
            (*c).cmsg_level = libc::SOL_SOCKET;
            (*c).cmsg_type = libc::SCM_RIGHTS;
            (*c).cmsg_len = libc::CMSG_LEN(std::mem::size_of::<RawFd>() as u32) as _;
            std::ptr::write_unaligned(libc::CMSG_DATA(c) as *mut RawFd, fd);
        }
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
    match recv_msg(sock)? {
        None => Ok(None),
        Some((Some(fd), data)) => Ok(Some((fd, data))),
        Some((None, _)) => Err(io::Error::new(io::ErrorKind::InvalidData, "message without a descriptor")),
    }
}

/// One message of [`send_msg`]: its descriptor if it carried one, and its data. `Ok(None)` = the peer closed the
/// socket. The whole message is read either way, so the next one starts at its own header.
pub fn recv_msg(sock: &impl AsRawFd) -> io::Result<Option<(Option<OwnedFd>, Vec<u8>)>> {
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
    let len = u32::from_le_bytes(head) as usize;
    if len > MAX_DATA {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "data too large"));
    }
    let mut data = vec![0u8; len];
    read_exact(sock.as_raw_fd(), &mut data)?;
    Ok(Some((fd, data)))
}

/// A zygote's status record on its control socket (zygote -> front; the hand-overs go the other way): u32 hand-overs
/// taken + u32 matches running, both little-endian.
pub const STATUS_LEN: usize = 8;

/// Zygote: send its status without blocking. `Ok(false)` = the socket's buffer is full (the front has not read for a
/// long time): nothing was sent, send it again later.
pub fn send_status(sock: &impl AsRawFd, taken: u32, running: u32) -> io::Result<bool> {
    let mut b = [0u8; STATUS_LEN];
    b[..4].copy_from_slice(&taken.to_le_bytes());
    b[4..].copy_from_slice(&running.to_le_bytes());
    loop {
        let n = unsafe { libc::send(sock.as_raw_fd(), b.as_ptr() as *const libc::c_void, STATUS_LEN, libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL) };
        if n < 0 {
            let e = io::Error::last_os_error();
            match e.kind() {
                io::ErrorKind::Interrupted => continue,
                io::ErrorKind::WouldBlock => return Ok(false),
                _ => return Err(e),
            }
        }
        // a record is never left cut: the rest goes out blocking
        if (n as usize) < STATUS_LEN {
            write_all(sock.as_raw_fd(), &b[n as usize..])?;
        }
        return Ok(true);
    }
}

/// Front: read the status records that arrived, without blocking, and return the latest (None = no new complete
/// record). `pending` keeps a cut record for the next call.
pub fn read_status(sock: &impl AsRawFd, pending: &mut Vec<u8>) -> io::Result<Option<(u32, u32)>> {
    let mut buf = [0u8; 512];
    loop {
        let n = unsafe { libc::recv(sock.as_raw_fd(), buf.as_mut_ptr() as *mut libc::c_void, buf.len(), libc::MSG_DONTWAIT) };
        if n < 0 {
            let e = io::Error::last_os_error();
            match e.kind() {
                io::ErrorKind::Interrupted => continue,
                io::ErrorKind::WouldBlock => break,
                _ => return Err(e),
            }
        }
        if n == 0 {
            // the zygote closed its end (it exited): no more records
            break;
        }
        pending.extend_from_slice(&buf[..n as usize]);
    }
    Ok(last_status(pending))
}

/// The last complete status record in `pending`; every complete record is removed, a cut one stays.
pub fn last_status(pending: &mut Vec<u8>) -> Option<(u32, u32)> {
    let whole = pending.len() / STATUS_LEN * STATUS_LEN;
    if whole == 0 {
        return None;
    }
    let r = &pending[whole - STATUS_LEN..whole];
    let s = (u32::from_le_bytes([r[0], r[1], r[2], r[3]]), u32::from_le_bytes([r[4], r[5], r[6], r[7]]));
    pending.drain(..whole);
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    #[test]
    fn the_latest_complete_status_record_wins() {
        let rec = |a: u32, b: u32| [a.to_le_bytes(), b.to_le_bytes()].concat();
        let mut p = Vec::new();
        assert_eq!(last_status(&mut p), None);
        p.extend_from_slice(&rec(1, 1));
        p.extend_from_slice(&rec(2, 0));
        p.extend_from_slice(&rec(3, 1)[..5]);
        assert_eq!(last_status(&mut p), Some((2, 0)), "the cut third record waits");
        assert_eq!(p.len(), 5);
        p.extend_from_slice(&rec(3, 1)[5..]);
        assert_eq!(last_status(&mut p), Some((3, 1)));
        assert!(p.is_empty());
    }

    #[test]
    fn status_records_cross_the_control_socket_both_ways() {
        let (front, zygote) = UnixStream::pair().unwrap();
        let mut pending = Vec::new();
        assert_eq!(read_status(&front, &mut pending).unwrap(), None, "nothing sent yet: no block");
        assert!(send_status(&zygote, 1, 1).unwrap());
        assert!(send_status(&zygote, 1, 0).unwrap());
        assert_eq!(read_status(&front, &mut pending).unwrap(), Some((1, 0)));
        // a hand-over still goes the other way on the same socket
        let (_x, y) = UnixStream::pair().unwrap();
        send_fd(&front, y.as_raw_fd(), b"create").unwrap();
        assert!(send_status(&zygote, 2, 1).unwrap());
        let (_fd, data) = recv_fd(&zygote).unwrap().unwrap();
        assert_eq!(data, b"create");
        assert_eq!(read_status(&front, &mut pending).unwrap(), Some((2, 1)));
        drop(zygote);
        assert_eq!(read_status(&front, &mut pending).unwrap(), None, "a closed zygote end: no record, no error");
    }

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
    fn a_message_without_a_descriptor_crosses_and_the_next_one_stays_apart() {
        let (a, b) = UnixStream::pair().unwrap();
        let (_x, y) = UnixStream::pair().unwrap();
        send_msg(&a, None, b"failed: no room").unwrap();
        send_msg(&a, Some(y.as_raw_fd()), b"loaded").unwrap();
        let (f1, d1) = recv_msg(&b).unwrap().unwrap();
        assert!(f1.is_none());
        assert_eq!(d1, b"failed: no room");
        let (f2, d2) = recv_msg(&b).unwrap().unwrap();
        assert!(f2.is_some());
        assert_eq!(d2, b"loaded");
        send_msg(&a, None, b"x").unwrap();
        assert!(recv_fd(&b).is_err(), "recv_fd wants a descriptor");
        send_fd(&a, y.as_raw_fd(), b"after").unwrap();
        assert_eq!(recv_fd(&b).unwrap().unwrap().1, b"after", "the refused message was read whole");
    }

    #[test]
    fn a_closed_control_socket_is_none() {
        let (a, b) = UnixStream::pair().unwrap();
        drop(a);
        assert!(recv_fd(&b).unwrap().is_none());
    }
}
