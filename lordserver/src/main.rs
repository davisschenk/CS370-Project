use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use futures_channel::mpsc::{unbounded, UnboundedSender};
use futures_util::{StreamExt, pin_mut};
use lordserial::parser::LordParser;
use tokio::{net::{TcpListener, TcpStream}, select};
use tokio_tungstenite::{accept_async, tungstenite::protocol::Message};

const SERVER_ADDRESS: &str = "127.0.0.1:8080";

type Ms = UnboundedSender<Message>;
type Peers = Arc<Mutex<HashMap<SocketAddr, Ms>>>;

async fn handle_connection(peers: Peers, tcp: TcpStream, addr: SocketAddr) {
    let websocket = accept_async(tcp)
        .await
        .expect("Failed to establish websocket");
    println!("Connected: {:?}", addr);
    let (tx, rx) = unbounded();
    // Add client to our map of peers
    peers.lock().unwrap().insert(addr, tx);

    let (ws_write, _ws_read) = websocket.split();

    // We dont care if this returns an error or is ok, disconnect either way.
    let _ = rx.map(Ok)
        .forward(ws_write)
        .await;


    println!("Disconnected!");
    peers.lock().unwrap().remove(&addr);
}

fn broadcast_messages(peers: Peers) {
    let port = serialport::new("/dev/ttyACM0", 115200).open().unwrap();
    let mut parser = LordParser::new(port, move |packet| {
        for (peer_addr, peer_rx) in peers.lock().unwrap().iter() {
            let send_r = peer_rx
                .unbounded_send(Message::text(format!("{:?}", packet)));

            match send_r {
                Ok(()) => (),
                Err(_) => {
                    eprintln!("Failed to send message, removing client {}", peer_addr);
                    peers.lock().unwrap().remove(&peer_addr);
                }
            }
        }
    });
    parser.parse();
}

#[tokio::main]
async fn main() {
    let state = Peers::new(Mutex::new(HashMap::new()));
    let try_socket = TcpListener::bind(SERVER_ADDRESS).await;
    let listener = try_socket.expect("Failed to bind");

    let s = state.clone();
    tokio::task::spawn_blocking(move || broadcast_messages(s));

    while let Ok((stream, addr)) = listener.accept().await {
        tokio::spawn(handle_connection(state.clone(), stream, addr));
    }
}
