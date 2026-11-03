//! A blocking connection with vanilla's pipeline: optional AES/CFB8
//! encryption, VarInt21 framing and threshold compression. After login it
//! splits into a reader (for a receive thread) and a shareable writer. It runs
//! over TCP, or over an in-memory pipe between two threads of one process
//! (vanilla's `LocalChannel`, which an integrated server's player uses).
use crate::{Error, Result, buf::Reader, crypto::Cfb8, frame};
use std::{
    io::{Read, Write},
    net::{Shutdown, SocketAddr, TcpStream, ToSocketAddrs},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicI64, Ordering},
    },
    time::{Duration, Instant},
};

/// One direction of a [`Pipe`]: the bytes written and not read yet, and
/// whether either end closed it.
#[derive(Default)]
struct Channel {
    state: Mutex<(std::collections::VecDeque<u8>, bool)>,
    ready: Condvar,
}
impl Channel {
    fn close(&self) {
        self.state.lock().unwrap().1 = true;
        self.ready.notify_all();
    }
}
/// The channels of one end; dropping the end's last clone closes them, as
/// a socket closes with its process.
struct End {
    incoming: Arc<Channel>,
    outgoing: Arc<Channel>,
}
impl Drop for End {
    fn drop(&mut self) {
        self.incoming.close();
        self.outgoing.close();
    }
}
/// One end of an in-memory byte pipe: what one end writes the other reads.
pub struct Pipe {
    end: Arc<End>,
    /// The read timeout (`set_read_timeout`).
    timeout: Mutex<Option<Duration>>,
}
impl Pipe {
    /// Two connected ends.
    pub fn pair() -> (Self, Self) {
        let (a, b) = (Arc::new(Channel::default()), Arc::new(Channel::default()));
        let end = |incoming, outgoing| Self {
            end: Arc::new(End { incoming, outgoing }),
            timeout: Mutex::new(None),
        };
        (end(a.clone(), b.clone()), end(b, a))
    }
    fn try_clone(&self) -> Self {
        Self {
            end: self.end.clone(),
            timeout: Mutex::new(*self.timeout.lock().unwrap()),
        }
    }
    /// Close both directions: reads at either end then see the end of the stream.
    pub fn shutdown(&self) {
        self.end.incoming.close();
        self.end.outgoing.close();
    }
}
impl Read for Pipe {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let deadline = self.timeout.lock().unwrap().map(|t| Instant::now() + t);
        let mut state = self.end.incoming.state.lock().unwrap();
        while state.0.is_empty() && !state.1 {
            state = match deadline {
                None => self.end.incoming.ready.wait(state).unwrap(),
                Some(deadline) => {
                    let left = deadline.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return Err(std::io::ErrorKind::TimedOut.into());
                    }
                    self.end.incoming.ready.wait_timeout(state, left).unwrap().0
                }
            };
        }
        let n = out.len().min(state.0.len());
        for (o, b) in out.iter_mut().zip(state.0.drain(..n)) {
            *o = b;
        }
        Ok(n)
    }
}
impl Write for Pipe {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        let mut state = self.end.outgoing.state.lock().unwrap();
        if state.1 {
            return Err(std::io::ErrorKind::BrokenPipe.into());
        }
        state.0.extend(data);
        self.end.outgoing.ready.notify_all();
        Ok(data.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// What a connection runs over.
enum Stream {
    Tcp(TcpStream),
    Memory(Pipe),
}
impl Stream {
    fn try_clone(&self) -> std::io::Result<Self> {
        Ok(match self {
            Self::Tcp(s) => Self::Tcp(s.try_clone()?),
            Self::Memory(p) => Self::Memory(p.try_clone()),
        })
    }
    fn set_read_timeout(&self, timeout: Option<Duration>) -> std::io::Result<()> {
        match self {
            Self::Tcp(s) => s.set_read_timeout(timeout),
            Self::Memory(p) => {
                *p.timeout.lock().unwrap() = timeout;
                Ok(())
            }
        }
    }
    fn shutdown(&self) {
        match self {
            Self::Tcp(s) => {
                let _ = s.shutdown(Shutdown::Both);
            }
            Self::Memory(p) => p.shutdown(),
        }
    }
}
impl Read for Stream {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Tcp(s) => s.read(out),
            Self::Memory(p) => p.read(out),
        }
    }
}
impl Write for Stream {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Tcp(s) => s.write(data),
            Self::Memory(p) => p.write(data),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Tcp(s) => s.flush(),
            Self::Memory(p) => p.flush(),
        }
    }
}

/// Packets received/sent and bytes in/out, for the debug screen.
#[derive(Debug, Default)]
pub struct Counters {
    pub received: AtomicI64,
    pub sent: AtomicI64,
    pub bytes_in: AtomicI64,
    pub bytes_out: AtomicI64,
}

pub struct ConnReader {
    stream: Stream,
    buffer: Vec<u8>,
    decrypt: Option<Cfb8>,
    pub threshold: Option<usize>,
    pub counters: Arc<Counters>,
}
/// Closes a connection from another thread: a read or a write blocked on it fails.
pub struct Closer(Stream);
impl Closer {
    pub fn close(&self) {
        self.0.shutdown();
    }
}

pub struct ConnWriter {
    stream: Stream,
    encrypt: Option<Cfb8>,
    pub threshold: Option<usize>,
    counters: Arc<Counters>,
    pending: Vec<u8>,
    batching: bool,
}
static COALESCING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn set_coalescing(on: bool) {
    COALESCING.store(on, Ordering::Relaxed);
}
pub fn coalescing() -> bool {
    COALESCING.load(Ordering::Relaxed)
}
const BATCH_LIMIT: usize = 16 * 1024;
/// `InetAddress.getByName` under Java's default `java.net.preferIPv6Addresses=false`: IPv4
/// addresses first, whatever order the system resolver gives (macOS lists IPv6 first). Vanilla
/// connects to that first address only; the rest stay here as fallbacks.
fn ipv4_first(mut addrs: Vec<SocketAddr>) -> Vec<SocketAddr> {
    addrs.sort_by_key(SocketAddr::is_ipv6);
    addrs
}
/// RFC 8305's connection attempt delay: how long an address has alone before the next
/// one is tried beside it.
const ATTEMPT_DELAY: Duration = Duration::from_millis(250);
/// Both halves before the split.
pub struct Connection {
    pub reader: ConnReader,
    pub writer: ConnWriter,
}
impl Connection {
    pub fn connect(host: &str, port: u16, timeout: Duration) -> Result<Self> {
        let addrs: Vec<_> = (host, port)
            .to_socket_addrs()
            .map_err(|e| Error::new(format!("Unknown host {host}: {e}")))?
            .collect();
        Self::connect_addresses(host, addrs, timeout)
    }
    /// Connect through Minecraft's SRV redirect. Status requests still use the
    /// original address in their handshake (`ServerStatusPinger.pingServer`).
    pub fn connect_server(host: &str, port: u16, timeout: Duration) -> Result<Self> {
        Self::connect_server_with_address(host, port, timeout).map(|(connection, _, _)| connection)
    }
    /// `ConnectScreen.startConnecting`: login handshakes use the resolved host
    /// and port, including an SRV redirect, rather than the status request's address.
    pub fn connect_server_with_address(
        host: &str,
        port: u16,
        timeout: Duration,
    ) -> Result<(Self, String, u16)> {
        let resolved = crate::resolver::resolve(host, port)?;
        let connection = Self::connect_addresses(host, resolved.addresses, timeout)?;
        Ok((connection, resolved.host, resolved.port))
    }
    /// Each address starts once the earlier ones fail or [`ATTEMPT_DELAY`] after the last
    /// start, alongside those still trying, and the first to connect wins: a dead address
    /// in a round-robin set costs 250 ms instead of the whole timeout.
    fn connect_addresses(host: &str, addrs: Vec<SocketAddr>, timeout: Duration) -> Result<Self> {
        let addrs = ipv4_first(addrs);
        let (send, recv) = std::sync::mpsc::channel();
        let mut errors: Vec<Option<std::io::Error>> = addrs.iter().map(|_| None).collect();
        let (mut started, mut pending) = (0, 0);
        loop {
            if let Some(&addr) = addrs.get(started) {
                let (send, index) = (send.clone(), started);
                let spawned = std::thread::Builder::new()
                    .name("horizon-connect".into())
                    .spawn(move || {
                        let _ = send.send((index, TcpStream::connect_timeout(&addr, timeout)));
                    });
                match spawned {
                    Ok(_) => pending += 1,
                    Err(e) => errors[index] = Some(e),
                }
                started += 1;
            }
            if pending == 0 {
                if started < addrs.len() {
                    continue;
                }
                break;
            }
            let reply = if started < addrs.len() {
                match recv.recv_timeout(ATTEMPT_DELAY) {
                    Ok(reply) => reply,
                    Err(_) => continue,
                }
            } else {
                recv.recv().expect("a sender is held here")
            };
            pending -= 1;
            match reply {
                (_, Ok(stream)) => return Self::from_stream(stream),
                (index, Err(e)) => errors[index] = Some(e),
            }
        }
        // Report the first address's failure: it is the one vanilla tries, and the IPv6
        // fallbacks after it often fail only because the client has no IPv6 route.
        match errors.into_iter().flatten().next() {
            Some(e) => Err(Error::new(format!("Connection refused: {e}"))),
            None => Err(Error::new(format!("Unknown host {host}"))),
        }
    }
    pub fn from_stream(stream: TcpStream) -> Result<Self> {
        stream.set_nodelay(true)?;
        Self::over(Stream::Tcp(stream))
    }
    /// A connection over one end of an in-memory [`Pipe`].
    pub fn local(pipe: Pipe) -> Self {
        Self::over(Stream::Memory(pipe)).expect("a pipe end clones")
    }
    fn over(stream: Stream) -> Result<Self> {
        let counters = Arc::new(Counters::default());
        let write = stream.try_clone()?;
        Ok(Self {
            reader: ConnReader {
                stream,
                buffer: Vec::new(),
                decrypt: None,
                threshold: None,
                counters: counters.clone(),
            },
            writer: ConnWriter {
                stream: write,
                encrypt: None,
                threshold: None,
                counters,
                pending: Vec::new(),
                batching: false,
            },
        })
    }
    /// `Connection.isMemoryConnection`: through a [`Pipe`] rather than a socket.
    pub fn is_memory(&self) -> bool {
        matches!(self.reader.stream, Stream::Memory(_))
    }
    /// ClientboundLoginCompressionPacket: negative thresholds disable compression.
    pub fn set_compression(&mut self, threshold: i32) {
        let t = usize::try_from(threshold).ok();
        self.reader.threshold = t;
        self.writer.threshold = t;
    }
    /// Connection.setEncryptionKey: both directions keyed with the shared secret.
    pub fn enable_encryption(&mut self, secret: &[u8; 16]) {
        let mut decrypt = Cfb8::new(secret, secret);
        // Bytes already read past the last packet were sent encrypted.
        decrypt.decrypt(&mut self.reader.buffer);
        self.reader.decrypt = Some(decrypt);
        self.writer.encrypt = Some(Cfb8::new(secret, secret));
    }
    pub fn send(&mut self, body: &[u8]) -> Result<()> {
        self.writer.send(body)
    }
    pub fn recv(&mut self) -> Result<(i32, Vec<u8>)> {
        self.reader.recv()
    }
    pub fn split(self) -> (ConnReader, ConnWriter) {
        (self.reader, self.writer)
    }
}
impl ConnReader {
    /// Read exactly `len` bytes as they are, with no framing.
    pub fn read_raw(&mut self, len: usize) -> Result<Vec<u8>> {
        let mut out = vec![0u8; len];
        self.stream.read_exact(&mut out)?;
        Ok(out)
    }
    /// Block until a whole packet arrives: (id, payload).
    pub fn recv(&mut self) -> Result<(i32, Vec<u8>)> {
        loop {
            if let Some(frame) = frame::split(&mut self.buffer)? {
                let body = frame::decompress(frame, self.threshold)?;
                if body.is_empty() {
                    // A frame with no packet in it. Vanilla's PacketDecoder
                    // only decodes a packet from a readable buffer, so an
                    // empty compressed frame is skipped and the connection
                    // stays up.
                    continue;
                }
                let mut r = Reader::new(&body);
                let id = r.varint()?;
                let start = r.position();
                self.counters.received.fetch_add(1, Ordering::Relaxed);
                return Ok((id, body[start..].to_vec()));
            }
            let mut chunk = [0u8; 16384];
            let n = match self.stream.read(&mut chunk) {
                Ok(0) => return Err(Error::end_of_stream()),
                Ok(n) => n,
                // The read timeout passed: Windows reports TimedOut, Unix
                // WouldBlock.
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) =>
                {
                    return Err(Error::timed_out());
                }
                Err(e) => return Err(e.into()),
            };
            self.counters
                .bytes_in
                .fetch_add(n as i64, Ordering::Relaxed);
            let data = &mut chunk[..n];
            if let Some(c) = &mut self.decrypt {
                c.decrypt(data);
            }
            self.buffer.extend_from_slice(data);
        }
    }
    /// A [`Closer`] for this connection.
    pub fn closer(&self) -> Result<Closer> {
        Ok(Closer(self.stream.try_clone()?))
    }
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> Result<()> {
        Ok(self.stream.set_read_timeout(timeout)?)
    }
}
impl ConnWriter {
    /// Write bytes as they are, with no framing: the legacy server list ping
    /// (`LegacyServerPinger`) is not a framed protocol.
    pub fn write_raw(&mut self, data: &[u8]) -> Result<()> {
        self.stream.write_all(data)?;
        self.stream.flush()?;
        Ok(())
    }
    /// Send one packet body (VarInt id + payload).
    pub fn send(&mut self, body: &[u8]) -> Result<()> {
        self.send_contents(&frame::contents(body, self.threshold)?)
    }
    /// Send a frame whose contents [`frame::contents`] made with this
    /// writer's threshold.
    pub fn send_contents(&mut self, contents: &[u8]) -> Result<()> {
        let mut framed = frame::prefixed(contents);
        if let Some(c) = &mut self.encrypt {
            c.encrypt(&mut framed);
        }
        self.counters.sent.fetch_add(1, Ordering::Relaxed);
        self.counters
            .bytes_out
            .fetch_add(framed.len() as i64, Ordering::Relaxed);
        if self.batching {
            self.pending.extend_from_slice(&framed);
            if self.pending.len() >= BATCH_LIMIT {
                return self.flush();
            }
            return Ok(());
        }
        self.stream.write_all(&framed)?;
        Ok(())
    }
    /// Hold small packets until [`Self::flush`] and write them as one segment train;
    /// turning it off writes what is held. The bytes on the wire are the same.
    pub fn set_batching(&mut self, on: bool) -> Result<()> {
        self.batching = on && !matches!(self.stream, Stream::Memory(_));
        if self.batching { Ok(()) } else { self.flush() }
    }
    pub fn send_now(&mut self, body: &[u8]) -> Result<()> {
        self.send(body)?;
        self.flush()
    }
    pub fn flush(&mut self) -> Result<()> {
        if !self.pending.is_empty() {
            let result = self.stream.write_all(&self.pending);
            self.pending.clear();
            result?;
        }
        Ok(())
    }
    pub fn shutdown(&self) {
        self.stream.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn ipv4_first_keeps_each_familys_order() {
        let a = |s: &str| s.parse::<SocketAddr>().unwrap();
        let resolved = vec![
            a("[2001:db8::1]:25565"),
            a("192.0.2.1:25565"),
            a("[2001:db8::2]:25565"),
            a("10.0.0.1:25565"),
        ];
        assert_eq!(
            ipv4_first(resolved),
            [
                a("192.0.2.1:25565"),
                a("10.0.0.1:25565"),
                a("[2001:db8::1]:25565"),
                a("[2001:db8::2]:25565"),
            ]
        );
    }

    fn peer(c: &Connection) -> SocketAddr {
        match &c.reader.stream {
            Stream::Tcp(s) => s.peer_addr().unwrap(),
            Stream::Memory(_) => panic!("not a socket"),
        }
    }

    /// Binds `[::1]`, or `None` where the machine has no IPv6 loopback.
    fn ipv6_loopback() -> Option<TcpListener> {
        let listener = TcpListener::bind("[::1]:0");
        if listener.is_err() {
            eprintln!("skipped: no IPv6 loopback");
        }
        listener.ok()
    }

    #[test]
    fn connects_to_ipv4_although_the_resolver_lists_ipv6_first() {
        // Both listen, as with a server whose IPv6 address accepts and then resets.
        let Some(v6_listener) = ipv6_loopback() else {
            return;
        };
        let v4_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let v6 = v6_listener.local_addr().unwrap();
        let v4 = v4_listener.local_addr().unwrap();
        let c =
            Connection::connect_addresses("test", vec![v6, v4], Duration::from_secs(5)).unwrap();
        assert_eq!(peer(&c), v4);
    }

    #[test]
    fn falls_back_to_ipv6_when_ipv4_refuses() {
        let Some(v6_listener) = ipv6_loopback() else {
            return;
        };
        let v6 = v6_listener.local_addr().unwrap();
        let closed = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let c = Connection::connect_addresses("test", vec![v6, closed], Duration::from_secs(5))
            .unwrap();
        assert_eq!(peer(&c), v6);
    }

    #[test]
    fn a_dead_address_does_not_hold_up_the_next() {
        // TEST-NET-1 never answers (or is unreachable at once).
        let dead: SocketAddr = "192.0.2.1:25565".parse().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let live = listener.local_addr().unwrap();
        let start = Instant::now();
        let c = Connection::connect_addresses("test", vec![dead, live], Duration::from_secs(10))
            .unwrap();
        assert_eq!(peer(&c), live);
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "{:?}",
            start.elapsed()
        );
    }

    #[test]
    fn every_failure_reports_the_first_address() {
        let closed = |_| {
            TcpListener::bind("127.0.0.1:0")
                .unwrap()
                .local_addr()
                .unwrap()
        };
        let error = |host, addrs| {
            Connection::connect_addresses(host, addrs, Duration::from_secs(5))
                .err()
                .unwrap()
                .to_string()
        };
        let refused = error("test", (0..3).map(closed).collect());
        assert!(refused.starts_with("Connection refused"), "{refused}");
        assert_eq!(error("nowhere", Vec::new()), "Unknown host nowhere");
    }

    #[test]
    fn a_silent_peer_times_out_and_a_closed_one_ends_the_stream() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let mut c = Connection::connect("127.0.0.1", port, Duration::from_secs(5)).unwrap();
        let (server, _) = listener.accept().unwrap();
        c.reader
            .set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        let silent = c.recv().unwrap_err();
        assert!(silent.is_timed_out(), "{silent}");
        assert_eq!(silent.to_string(), "Timed out");
        drop(server);
        let closed = c.recv().unwrap_err();
        assert!(closed.is_end_of_stream(), "{closed}");
    }

    #[test]
    fn loopback_with_compression_and_encryption() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut c = Connection::from_stream(stream).unwrap();
            let (id, payload) = c.recv().unwrap();
            assert_eq!((id, payload.as_slice()), (0, &b"plain"[..]));
            c.enable_encryption(&[7; 16]);
            c.set_compression(64);
            let (id, payload) = c.recv().unwrap();
            assert_eq!((id, payload.len()), (5, 1000));
            let mut body = vec![9];
            body.extend_from_slice(b"reply");
            c.send(&body).unwrap();
        });
        let mut c = Connection::connect("127.0.0.1", port, Duration::from_secs(5)).unwrap();
        let mut body = vec![0];
        body.extend_from_slice(b"plain");
        c.send(&body).unwrap();
        c.enable_encryption(&[7; 16]);
        c.set_compression(64);
        let mut big = vec![5];
        big.extend(std::iter::repeat_n(3u8, 1000));
        c.send(&big).unwrap();
        let (id, payload) = c.recv().unwrap();
        assert_eq!((id, payload.as_slice()), (9, &b"reply"[..]));
        server.join().unwrap();
    }

    #[test]
    fn local_pipe_carries_packets_and_closes() {
        let (a, b) = Pipe::pair();
        let server = std::thread::spawn(move || {
            let mut c = Connection::local(b);
            c.set_compression(64);
            let (id, payload) = c.recv().unwrap();
            assert_eq!((id, payload.len()), (5, 1000));
            c.send(&[9, 1, 2]).unwrap();
            let (_, writer) = c.split();
            writer.shutdown();
        });
        let mut c = Connection::local(a);
        c.set_compression(64);
        let mut big = vec![5];
        big.extend(std::iter::repeat_n(3u8, 1000));
        c.send(&big).unwrap();
        assert_eq!(c.recv().unwrap(), (9, vec![1, 2]));
        assert_eq!(c.reader.counters.sent.load(Ordering::Relaxed), 1);
        assert_eq!(c.reader.counters.received.load(Ordering::Relaxed), 1);
        server.join().unwrap();
        assert!(c.recv().is_err());
    }

    #[test]
    fn dropping_a_pipe_end_closes_it() {
        let (a, b) = Pipe::pair();
        let mut c = Connection::local(a);
        drop(b);
        assert!(c.recv().is_err());
        assert!(c.send(&[1]).is_err());
    }

    #[test]
    fn local_pipe_read_times_out() {
        let (a, _b) = Pipe::pair();
        let mut c = Connection::local(a);
        c.reader
            .set_read_timeout(Some(Duration::from_millis(20)))
            .unwrap();
        assert!(c.recv().is_err());
    }

    /// Encrypted bytes that arrive in the same read as the last plain packet
    /// are decrypted once encryption is enabled.
    #[test]
    fn encrypted_bytes_read_before_enabling_encryption() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (sent, both_sent) = std::sync::mpsc::channel();
        let client = std::thread::spawn(move || {
            let mut c = Connection::connect("127.0.0.1", port, Duration::from_secs(5)).unwrap();
            c.send(&[0, 1, 2]).unwrap();
            c.enable_encryption(&[7; 16]);
            c.send(&[5, 4, 3]).unwrap();
            sent.send(c).unwrap();
        });
        let (stream, _) = listener.accept().unwrap();
        let mut c = Connection::from_stream(stream).unwrap();
        let _keep_open = both_sent.recv().unwrap();
        assert_eq!(c.recv().unwrap(), (0, vec![1, 2]));
        c.enable_encryption(&[7; 16]);
        assert_eq!(c.recv().unwrap(), (5, vec![4, 3]));
        client.join().unwrap();
    }

    /// A server may send a compressed frame that carries no packet: the
    /// uncompressed length is 0 and the frame holds nothing else. Vanilla
    /// skips it (its PacketDecoder only reads a packet from a readable
    /// buffer), so the client stays connected.
    #[test]
    fn an_empty_compressed_frame_is_skipped() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut c = Connection::from_stream(stream).unwrap();
            c.set_compression(64);
            // The empty frame: its length prefix, then the uncompressed
            // length 0 and nothing else.
            c.writer.send_contents(&[0]).unwrap();
            let mut body = vec![3];
            body.extend_from_slice(b"after");
            c.writer.send(&body).unwrap();
        });
        let mut c = Connection::connect("127.0.0.1", port, Duration::from_secs(5)).unwrap();
        c.set_compression(64);
        let (id, payload) = c.recv().unwrap();
        assert_eq!((id, payload.as_slice()), (3, &b"after"[..]));
        server.join().unwrap();
    }
    #[test]
    fn batched_packets_wait_for_the_flush_and_arrive_in_order() {
        let (a, b) = Pipe::pair();
        let (_unused, mut writer) = Connection::local(a).split();
        let mut reader = Connection::local(b);
        reader
            .reader
            .set_read_timeout(Some(Duration::from_millis(30)))
            .unwrap();
        writer.set_batching(true).unwrap();
        writer.batching = true;
        writer.send(&[1, 0xAA]).unwrap();
        writer.send(&[2, 0xBB, 0xCC]).unwrap();
        assert!(reader.recv().unwrap_err().is_timed_out());
        writer.flush().unwrap();
        assert_eq!(reader.recv().unwrap(), (1, vec![0xAA]));
        assert_eq!(reader.recv().unwrap(), (2, vec![0xBB, 0xCC]));
        writer.send(&[3]).unwrap();
        writer.set_batching(false).unwrap();
        assert_eq!(reader.recv().unwrap(), (3, vec![]));
        writer.send(&[4]).unwrap();
        assert_eq!(reader.recv().unwrap(), (4, vec![]));
    }

    #[test]
    fn send_now_reaches_the_peer_while_batching_is_on() {
        let (a, b) = Pipe::pair();
        let (_unused, mut writer) = Connection::local(a).split();
        let mut reader = Connection::local(b);
        writer.batching = true;
        writer.send(&[1, 0xAA]).unwrap();
        writer.send_now(&[2, 0xBB]).unwrap();
        assert_eq!(reader.recv().unwrap(), (1, vec![0xAA]));
        assert_eq!(reader.recv().unwrap(), (2, vec![0xBB]));
    }

    #[test]
    fn a_full_batch_is_written_without_a_flush() {
        let (a, b) = Pipe::pair();
        let (_unused, mut writer) = Connection::local(a).split();
        let mut reader = Connection::local(b);
        writer.batching = true;
        let mut big = vec![7];
        big.resize(BATCH_LIMIT + 10, 0x55);
        writer.send(&big).unwrap();
        let (id, payload) = reader.recv().unwrap();
        assert_eq!((id, payload.len()), (7, BATCH_LIMIT + 9));
    }

    #[test]
    fn batched_packets_keep_the_encrypted_stream_and_compression_intact() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut c = Connection::from_stream(stream).unwrap();
            c.enable_encryption(&[3; 16]);
            c.set_compression(32);
            c.writer.set_batching(true).unwrap();
            c.send(&[1, 2]).unwrap();
            let mut long = vec![9];
            long.resize(300, 4);
            c.send(&long).unwrap();
            c.send(&[5]).unwrap();
            c.writer.flush().unwrap();
            c
        });
        let mut c = Connection::connect("127.0.0.1", port, Duration::from_secs(5)).unwrap();
        c.enable_encryption(&[3; 16]);
        c.set_compression(32);
        assert_eq!(c.recv().unwrap(), (1, vec![2]));
        let (id, payload) = c.recv().unwrap();
        assert_eq!((id, payload.len(), payload[0]), (9, 299, 4));
        assert_eq!(c.recv().unwrap(), (5, vec![]));
        let _keep = server.join().unwrap();
    }

    #[test]
    fn every_tcp_connection_sets_nodelay() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let c = Connection::connect("127.0.0.1", port, Duration::from_secs(5)).unwrap();
        let (stream, _) = listener.accept().unwrap();
        let s = Connection::from_stream(stream).unwrap();
        for conn in [&c, &s] {
            match &conn.reader.stream {
                Stream::Tcp(t) => assert!(t.nodelay().unwrap()),
                Stream::Memory(_) => panic!("not a socket"),
            }
        }
    }

    fn burst_round_trip(client: TcpStream, mut server: TcpStream, rounds: usize) -> Duration {
        let echo = std::thread::spawn(move || {
            let mut got = [0u8; 2];
            for _ in 0..rounds {
                server.read_exact(&mut got).unwrap();
                server.write_all(&[1]).unwrap();
            }
        });
        let mut client = client;
        let start = Instant::now();
        for _ in 0..rounds {
            client.write_all(&[7]).unwrap();
            client.write_all(&[8]).unwrap();
            client.read_exact(&mut [0u8; 1]).unwrap();
        }
        let took = start.elapsed();
        echo.join().unwrap();
        took / rounds as u32
    }

    fn socket_pair(nodelay: bool) -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (server, _) = listener.accept().unwrap();
        for s in [&client, &server] {
            s.set_nodelay(nodelay).unwrap();
        }
        (client, server)
    }

    /// A player action is a few small packets in a row (swing, use, tick end). With Nagle on,
    /// the second waits for the peer's delayed ACK; with TCP_NODELAY it leaves at once.
    #[test]
    fn nodelay_round_trip_of_a_small_packet_burst_stays_fast() {
        let (c, s) = socket_pair(false);
        let nagle = burst_round_trip(c, s, 5);
        let (c, s) = socket_pair(true);
        let nodelay = burst_round_trip(c, s, 5);
        eprintln!("burst round trip: nagle {nagle:?}, nodelay {nodelay:?}");
        assert!(nodelay < Duration::from_millis(20), "{nodelay:?}");
    }
    fn action_round_trip(
        mut client: Connection,
        mut server: Connection,
        batched: bool,
    ) -> Duration {
        let echo = std::thread::spawn(move || {
            for _ in 0..200 {
                for expected in [1, 2, 3] {
                    assert_eq!(server.recv().unwrap().0, expected);
                }
                server.send(&[9]).unwrap();
            }
        });
        client.writer.set_batching(batched).unwrap();
        let mut times = Vec::new();
        for _ in 0..200 {
            let start = Instant::now();
            client.send(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).unwrap();
            client.send(&[2, 0]).unwrap();
            client.send(&[3]).unwrap();
            client.writer.flush().unwrap();
            assert_eq!(client.recv().unwrap().0, 9);
            times.push(start.elapsed());
        }
        echo.join().unwrap();
        times.sort();
        times[times.len() / 2]
    }

    /// A block break or an attack is three small packets (action, swing, tick end) answered by
    /// the server: through memory (the integrated server) and through a socket (LAN, dedicated).
    #[test]
    fn loopback_round_trip_of_a_player_action_is_the_same_with_and_without_batching() {
        for batched in [false, true] {
            let (a, b) = Pipe::pair();
            let memory = action_round_trip(Connection::local(a), Connection::local(b), batched);
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let client = Connection::connect("127.0.0.1", port, Duration::from_secs(5)).unwrap();
            let server = Connection::from_stream(listener.accept().unwrap().0).unwrap();
            let socket = action_round_trip(client, server, batched);
            eprintln!("action round trip, batched {batched}: memory {memory:?}, socket {socket:?}");
            assert!(memory < Duration::from_millis(5), "{memory:?}");
            assert!(socket < Duration::from_millis(5), "{socket:?}");
        }
    }
}
