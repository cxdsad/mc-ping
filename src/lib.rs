#![crate_type = "rlib"]
#[doc = include_str!("../README.md")]

pub mod connection;
pub mod packets;
mod varint;
pub mod mc_text;

#[tokio::test]
async fn test_localhost() {
    use crate::connection::Connection;
    let mut conn = Connection::connect(("127.0.0.1".to_string(), 25565)).await.unwrap();
    conn.send_handshake().await.unwrap();
    let status = conn.ping().await.unwrap();
    println!("{:?}", status);
}