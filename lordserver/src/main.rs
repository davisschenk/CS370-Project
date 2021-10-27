use clap::{App, Arg};
use futures_channel::mpsc::{unbounded, UnboundedSender};
use futures_util::StreamExt;
use lordserial::{data::DataPacket, parser::LordParser};
use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufRead, Write},
    net::SocketAddr,
    str::FromStr,
    sync::{Arc, Mutex},
};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::protocol::Message};

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
    let _ = rx.map(Ok).forward(ws_write).await;

    println!("Disconnected!");
    peers.lock().unwrap().remove(&addr);
}

fn broadcast_messages(peers: Peers, port: String) {
    let port = serialport::new(port, 115200).open().unwrap();
    let mut parser = LordParser::new(port, move |packet| {
        for (_peer_addr, peer_rx) in peers.lock().unwrap().iter() {
            let json = match DataPacket::new(&packet) {
                DataPacket::IMU(d) => serde_json::to_string(&d).unwrap(),
                DataPacket::GNSS(d) => serde_json::to_string(&d).unwrap(),
                _ => break, // Dont send unimplemented packets
            };

            let _ = peer_rx.unbounded_send(Message::text(json));
        }
    });
    parser.parse();
}

fn recorder(peers: Peers, file_path: String) {
    let (tx, mut rx) = unbounded();
    let addr = SocketAddr::from_str("0.0.0.0:0000").unwrap();
    peers.lock().unwrap().insert(addr, tx);

    let mut file = File::create(file_path).expect("File already exists");

    loop {
        match rx.try_next() {
            Ok(r) => match r {
                Some(t) => writeln!(file, "{}", t).unwrap(),
                None => break,
            },
            Err(_) => (),
        };
    }

    peers.lock().unwrap().remove(&addr);
}

fn simulator(peers: Peers, path: String) {
    let file = File::open(path).unwrap();

    loop {
        for line in io::BufReader::new(&file).lines() {
            if let Ok(line) = line {
                for (_peer_addr, peer_rx) in peers.lock().unwrap().iter() {
                    let _ = peer_rx.unbounded_send(Message::text(&line));
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let matches = App::new("Lord Server")
        .about("Parses data from a lord and sends it to clients connected to websockets")
        .arg(
            Arg::new("address")
                .long("address")
                .about("Address to run websocket server on")
                .default_value("127.0.0.1:8080"),
        )
        .arg(
            Arg::new("simulate")
                .long("simulate")
                .about("Send data from a file")
                .conflicts_with("port")
                .takes_value(true),
        )
        .arg(
            Arg::new("port")
                .long("port")
                .about("Port for lord")
                .default_value("/dev/ttyACM0"),
        )
        .arg(
            Arg::new("record")
                .long("record")
                .about("record to a file")
                .takes_value(true)
                .conflicts_with("simulate")
        )
        .get_matches();

    let state = Peers::new(Mutex::new(HashMap::new()));
    let try_socket = TcpListener::bind(matches.value_of("address").unwrap()).await;
    let listener = try_socket.expect("Failed to bind");

    // Read from a file and send to all peers
    if let Some(file) = matches.value_of("simulate") {
        let peers = state.clone();
        let file = file.to_string();
        tokio::task::spawn_blocking(move || simulator(peers, file));
    }
    
    // Read and parse messages from lord and send to all peers
    if let Some(port) = matches.value_of("port") {
        let s = state.clone();
        let port = port.to_string();
        tokio::task::spawn_blocking(move || broadcast_messages(s, port));
    }

    // Spawn a "peer" that puts all messages received into a file
    if let Some(file) = matches.value_of("record") {
        let peers = state.clone();
        let file = file.to_string();
        tokio::task::spawn_blocking(move || recorder(peers, file));
    }

    // Accept connections and spawn a new task that handles websocket connection
    while let Ok((stream, addr)) = listener.accept().await {
        tokio::spawn(handle_connection(state.clone(), stream, addr));
    }
}
