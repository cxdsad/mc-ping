use tokio::io::{AsyncWriteExt, AsyncReadExt};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;
use crate::mc_text::ServerStatus;
use crate::packets::{ClientHandshake, ServerQueryResponse, StatusQuery};
use anyhow::{anyhow, Result};
use tokio::net::lookup_host;


fn is_domain(addr: &str) -> bool {
    addr.parse::<std::net::IpAddr>().is_err()
}

/// Represents a TCP connection to a Minecraft server.
///
/// # Examples
///
/// ```no_run
/// use std::time::Duration;
/// # use anyhow::Result;
/// # #[tokio::main]
/// # async fn main() -> Result<()> {
/// let addr = ("play.example.com".to_string(), 25565);
/// let mut conn = Connection::connect(addr).await?;
/// let status = conn.ping().await?;
/// println!("Server status: {:?}", status);
/// # Ok(())
/// # }
/// ```
pub struct Connection {
    /// Underlying TCP stream.
    pub stream: TcpStream,

    /// Server address as (IP/domain, port).
    pub addr: (String, u16),
}

impl Connection {
    /// Connects to a Minecraft server at the specified address.
    ///
    /// If the `resolve` feature is enabled, attempts to resolve domain names to IP addresses before connecting.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection or DNS resolution fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use anyhow::Result;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// let addr = ("localhost".to_string(), 25565);
    /// let conn = Connection::connect(addr).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect(addr: (String, u16)) -> Result<Self> {
        #[cfg(not(feature = "resolve"))]
        {
            if is_domain(&addr.0) {
                return Err(anyhow!(r#"Enable feature "resolve" to enable domain resolving"#))
            }
            let stream = TcpStream::connect(addr.clone()).await?;
            Ok(Self {
                stream,
                addr,
            })
        }
        #[cfg(feature = "resolve")]
        {
            let host_port = format!("{}:{}", addr.0.clone(), addr.1);
            let mut addrs = lookup_host(host_port.clone()).await?;
            if let Some(sock_addr) = addrs.next() {
                let stream = TcpStream::connect(sock_addr).await?;
                Ok(Self {
                    stream,
                    addr: (addr.0, sock_addr.port()),
                })
            } else {
                Err(anyhow::anyhow!("Could not resolve address: {}", host_port))
            }
        }
    }

    /// Connects to a Minecraft server with a timeout.
    ///
    /// Attempts to connect and returns an error if the timeout elapses.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection fails or times out.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use std::time::Duration;
    /// # use anyhow::Result;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// let addr = ("example.com".to_string(), 25565);
    /// match Connection::connect_timeout(addr, Duration::from_secs(5)).await {
    ///     Ok(conn) => println!("Connected!"),
    ///     Err(e) => println!("Failed to connect: {}", e),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect_timeout(addr: (String, u16), _timeout: Duration) -> Result<Self> {
        let _conn = timeout(_timeout, Self::connect(addr)).await;
        match _conn {
            Ok(Ok(conn)) => Ok(conn),
            Err(err) => Err(anyhow::anyhow!("Could not connect: {} (timeout)", err))?,
            Ok(Err(err)) => Err(anyhow::anyhow!("Could not connect: {}", err))?,
        }
    }

    /// Sends the Minecraft handshake packet.
    ///
    /// This prepares the connection for further communication such as status query or login.
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the TCP stream fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use anyhow::Result;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// let addr = ("127.0.0.1".to_string(), 25565);
    /// let mut conn = Connection::connect(addr).await?;
    /// conn.send_handshake().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send_handshake(&mut self) -> Result<()> {
        let _ip = self.addr.0.clone();
        let _port = self.addr.1;
        let handshake = ClientHandshake::new(_ip, _port);
        let bytes = handshake.to_bytes();
        self.stream.write_all(bytes.as_slice()).await?;
        Ok(())
    }

    /// Sends the status query packet.
    ///
    /// Internal helper function, generally not called directly.
    async fn __send_query_packet(&mut self) -> Result<()> {
        let query = StatusQuery::new();
        let bytes = query.to_bytes();
        self.stream.write_all(bytes.as_slice()).await?;
        Ok(())
    }

    /// Reads and parses the server's status response packet.
    ///
    /// Internal helper function, generally not called directly.
    async fn __read_status_packet(&mut self) -> Result<ServerQueryResponse> {
        let mut buf = [0u8; 4096];
        self.stream.read(&mut buf).await?;
        let status_packet = ServerQueryResponse::from(&buf[..]);
        Ok(status_packet)
    }

    /// Queries the server status.
    ///
    /// Sends a status query and reads the response, returning a parsed ServerStatus.
    /// Assumes handshake has been sent beforehand.
    ///
    /// # Errors
    ///
    /// Returns an error if sending or receiving fails, or parsing fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use anyhow::Result;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// let addr = ("localhost".to_string(), 25565);
    /// let mut conn = Connection::connect(addr).await?;
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

    /// Performs a full ping: sends handshake + status query and returns server status.
    ///
    /// Convenient for a single-step status check.
    ///
    /// # Errors
    ///
    /// Returns an error if any network or parsing step fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use anyhow::Result;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// let addr = ("play.example.com".to_string(), 25565);
    /// let mut conn = Connection::connect(addr).await?;
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
