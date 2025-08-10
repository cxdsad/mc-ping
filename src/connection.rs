use tokio::io::{AsyncWriteExt, AsyncReadExt};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;
use crate::mc_text::ServerStatus;
use crate::packets::{ClientHandshake, ServerQueryResponse, StatusQuery};
use anyhow::{anyhow, Result};
use tokio::net::lookup_host;
use tokio_socks::tcp::Socks5Stream;

fn is_domain(addr: &str) -> bool {
    addr.parse::<std::net::IpAddr>().is_err()
}

/// Represents a TCP connection to a Minecraft server.
/// Supports optional SOCKS5 proxy connections.
///
/// # Type Parameters
///
/// * `T`: Underlying TCP stream type, usually `TcpStream`.
///
/// # Fields
///
/// * `stream`: Optionally holds the active TCP stream.
/// * `timeout`: Optional timeout duration in milliseconds for connection and I/O.
/// * `proxy_addr`: Optional SOCKS5 proxy address as `(host, port)`.
/// * `addr`: Target Minecraft server address `(host, port)`.
pub struct Connection<T> {
    pub is_initialized: bool,
    pub stream: Option<T>,
    pub timeout: Option<u64>,
    pub proxy_addr: Option<(String, u16)>,
    pub addr: (String, u16),
}

impl Connection<TcpStream> {
    /// Creates a new `Connection` instance with the target server address.
    ///
    /// The connection is not yet established.
    ///
    /// # Example
    ///
    /// ```
    /// # use anyhow::Result;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// use mc_ping::connection::Connection;
    ///
    /// let mut conn = Connection::new(("play.example.com".to_string(), 25565)).await;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(addr: (String, u16)) -> Self {
        Self {
            stream: None,
            timeout: None,
            is_initialized: true,
            proxy_addr: None,
            addr,
        }
    }

    /// Establishes a connection to the Minecraft server.
    ///
    /// If a SOCKS5 proxy is set via `proxy_socks5()`, the connection will be
    /// established through that proxy. Otherwise, it connects directly.
    ///
    /// DNS resolution depends on the "resolve" feature flag:
    /// - Without "resolve" feature: domain names are not supported (must be IP).
    /// - With "resolve" feature enabled: domain names are resolved asynchronously.
    ///
    /// # Errors
    ///
    /// Returns error if connection, proxy connection, or DNS resolution fails.
    ///
    /// # Example
    ///
    /// ```
    /// # use anyhow::Result;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// use mc_ping::connection::Connection;
    ///
    /// let mut conn = Connection::new(("example.com".to_string(), 25565)).await;
    /// conn = conn.connect().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect(&mut self) -> Result<Self> {
        let _timeout = self.timeout.unwrap_or(8000);

        #[cfg(not(feature = "resolve"))]
        {
            let addr = self.addr.clone();
            if is_domain(&addr.0) {
                return Err(anyhow!(r#"Enable feature "resolve" to enable domain resolving"#));
            }

            match &self.proxy_addr {
                None => {
                    // Direct TCP connection with timeout
                    let stream = timeout(Duration::from_millis(_timeout), TcpStream::connect(addr.clone())).await??;
                    Ok(Self {
                        stream: Some(stream),
                        is_initialized: true,
                        timeout: self.timeout.clone(),
                        proxy_addr: self.proxy_addr.clone(),
                        addr: self.addr.clone(),
                    })
                }
                Some(proxy_addr) => {
                    // Connect via SOCKS5 proxy with timeout
                    let stream = timeout(
                        Duration::from_millis(_timeout),
                        Socks5Stream::connect(
                            (proxy_addr.0.as_str(), proxy_addr.1),
                            (addr.0.as_str(), addr.1)
                        )
                    ).await??;
                    Ok(Self {
                        stream: Some(stream.into_inner()),
                        is_initialized: true,
                        timeout: self.timeout.clone(),
                        proxy_addr: self.proxy_addr.clone(),
                        addr: self.addr.clone(),
                    })
                }
            }
        }

        #[cfg(feature = "resolve")]
        {
            match &self.proxy_addr {
                Some(proxy_addr) => {
                    let stream = timeout(
                        Duration::from_millis(_timeout),
                        Socks5Stream::connect(
                            (proxy_addr.0.as_str(), proxy_addr.1),
                            (self.addr.0.as_str(), self.addr.1),
                        )
                    ).await??;

                    Ok(Self {
                        stream: Some(stream.into_inner()),
                        is_initialized: true,
                        timeout: self.timeout.clone(),
                        proxy_addr: self.proxy_addr.clone(),
                        addr: self.addr.clone(),
                    })
                }
                None => {
                    let host_port = format!("{}:{}", self.addr.0, self.addr.1);
                    let mut addrs = lookup_host(host_port).await?;
                    if let Some(sock_addr) = addrs.next() {
                        let stream = timeout(Duration::from_millis(_timeout), TcpStream::connect(sock_addr)).await??;
                        Ok(Self {
                            stream: Some(stream),
                            is_initialized: true,
                            timeout: self.timeout.clone(),
                            proxy_addr: None,
                            addr: self.addr.clone(),
                        })
                    } else {
                        Err(anyhow!("Could not resolve address: {}", self.addr.0))
                    }
                }
            }
        }
    }

    /// Sets the timeout for connection and I/O operations (milliseconds).
    ///
    /// # Errors
    ///
    /// Returns error if called before initialization.
    ///
    /// # Example
    ///
    /// ```
    /// # use anyhow::Result;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// use mc_ping::connection::Connection;
    ///
    /// let mut conn = Connection::new(("127.0.0.1".to_string(), 25565)).await;
    /// conn.timeout(5000).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn timeout(mut self, timeout: u64) -> Result<Self> {
        if !self.is_initialized {
            return Err(anyhow!("using: Connection::new((addr, port)).timeout(u64)"));
        }

        self.timeout = Some(timeout);
        Ok(self)
    }
    /// Sets the SOCKS5 proxy address to use for connections.
    ///
    /// # Errors
    ///
    /// Returns error if called before initialization.
    ///
    /// # Example
    ///
    /// ```
    /// # use anyhow::Result;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// use mc_ping::connection::Connection;
    ///
    /// let mut conn = Connection::new(("127.0.0.1".to_string(), 25565)).await;
    /// conn.proxy_socks5(("127.0.0.1".to_string(), 1080)).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn proxy_socks5(mut self, proxy_addr: (String, u16)) -> Result<Self> {
        if !self.is_initialized {
            return Err(anyhow!("using: Connection::new((ip, port)).proxy((ip, port))"));
        }

        self.proxy_addr = Some(proxy_addr);
        Ok(self)
    }

    /// Sends the Minecraft handshake packet to the server.
    ///
    /// This prepares the connection for status query or login.
    ///
    /// # Errors
    ///
    /// Returns error if the stream is not connected or writing fails.
    ///
    /// # Example
    ///
    /// ```
    /// # use anyhow::Result;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// use mc_ping::connection::Connection;
    ///
    /// let mut conn = Connection::new(("127.0.0.1".to_string(), 25565)).await;
    /// conn = conn.connect().await?;
    /// conn.send_handshake().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send_handshake(&mut self) -> Result<()> {
        let stream = match &mut self.stream {
            Some(s) => s,
            None => return Err(anyhow!("TCPstream is None. Maybe you forgot to .connect() ?")),
        };

        let ip = self.addr.0.clone();
        let port = self.addr.1;
        let handshake = ClientHandshake::new(ip, port);
        let bytes = handshake.to_bytes();

        timeout(
            Duration::from_millis(self.timeout.unwrap_or(9000)),
            stream.write_all(bytes.as_slice())
        ).await??;

        Ok(())
    }

    /// Internal helper to send the status query packet.
    ///
    /// # Errors
    ///
    /// Returns error if writing to stream fails or stream is not connected.
    async fn __send_query_packet(&mut self) -> Result<()> {
        let query = StatusQuery::new();
        let bytes = query.to_bytes();

        let stream = match &mut self.stream {
            Some(s) => s,
            None => return Err(anyhow!("TCPstream is None. Maybe you forgot to .connect()?")),
        };

        stream.write_all(bytes.as_slice()).await?;
        Ok(())
    }

    /// Internal helper to read the status response packet.
    ///
    /// # Errors
    ///
    /// Returns error if reading from stream fails or stream is not connected.
    async fn __read_status_packet(&mut self) -> Result<ServerQueryResponse> {
        let mut buf = [0u8; 10_000];

        let stream = match &mut self.stream {
            Some(s) => s,
            None => return Err(anyhow!("TCPstream is None. Maybe you forgot to .connect()?")),
        };

        let n = stream.read(&mut buf).await?;
        let status_packet = ServerQueryResponse::from(&buf[..n]).await;
        Ok(status_packet)
    }

    /// Sends a status query and reads the server response.
    ///
    /// Assumes handshake has already been sent.
    ///
    /// # Errors
    ///
    /// Returns error if sending or reading packets fails, or if parsing fails.
    ///
    /// # Example
    ///
    /// ```
    /// # use anyhow::Result;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// use mc_ping::connection::Connection;
    ///
    /// let mut conn = Connection::new(("localhost".to_string(), 25565)).await;
    /// conn = conn.connect().await?;
    /// conn.send_handshake().await?;
    /// let status = conn.get_status().await?;
    /// println!("Status: {:?}", status);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_status(&mut self) -> Result<ServerStatus> {
        self.__send_query_packet().await?;
        let _status = self.__read_status_packet().await?;
        Ok(_status.parse_status()?)
    }

    /// Performs a full ping: sends handshake, status query, and parses the response.
    ///
    /// Convenient for one-step status check.
    ///
    /// # Errors
    ///
    /// Returns error if any step (network or parsing) fails.
    ///
    /// # Example
    ///
    /// ```
    /// # use anyhow::Result;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// use mc_ping::connection::Connection;
    ///
    /// let mut conn = Connection::new(("play.example.com".to_string(), 25565)).await;
    /// conn = conn.connect().await?;
    /// let status = conn.ping().await?;
    /// println!("Server status: {:?}", status);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn ping(&mut self) -> Result<ServerStatus> {
        self.send_handshake().await?;
        self.__send_query_packet().await?;
        let status = self.__read_status_packet().await?;
        status.parse_status()
    }
}
