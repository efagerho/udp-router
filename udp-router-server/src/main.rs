use tokio::net::UdpSocket;
use tokio::io;

#[tokio::main]
async fn main() -> io::Result<()> {
    // Bind the socket to an address
    let socket = UdpSocket::bind("0.0.0.0:8888").await?;
    println!("Server listening on 0.0.0.0:8888");

    let mut buf = [0u8; 1024];

    loop {
        // Receive a message
        let (len, addr) = socket.recv_from(&mut buf).await?;
        let received = &buf[..len];
        socket.send_to(received, &addr).await?;
    }
}
