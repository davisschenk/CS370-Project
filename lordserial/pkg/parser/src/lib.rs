use ringbuf::RingBuffer;
use std::io::{Read, Write};
use packet::{Packet, PacketComponent};
use serialport::SerialPort;

enum State {
    SyncOne,
    SyncTwo,
    Descriptor,
    PayloadLength,
    Data { length: u8 },
}

pub struct LordParser<F>
where
    F: Fn(Packet),
{
    reader: Box<dyn SerialPort>,
    handler: F,
}

impl<F> LordParser<F>
where
    F: Fn(Packet),
{
    pub fn new(reader: Box<dyn SerialPort>, handler: F) -> Self {
        Self {
            reader: reader,
            handler,
        }
    }

    pub fn parse(&mut self) {
        let mut state = State::SyncOne;
        let buffer: RingBuffer<u8> = RingBuffer::new(512);
        let (mut producer, mut consumer) = buffer.split();

        loop {
            let mut current_packet: Vec<u8> = Vec::new();
            let mut building_packet = true;

            while building_packet {
                match consumer.pop() {
                    Some(curr_byte) => {
                        state = match (&state, curr_byte) {
                            (State::SyncOne, 0x75) => State::SyncTwo,
                            (State::SyncTwo, 0x65) => State::Descriptor,
                            (State::Descriptor, _) => State::PayloadLength,
                            (State::PayloadLength, len) => State::Data { length: len + 1 },
                            (State::Data { length }, _) if *length == 0 => {
                                building_packet = false;
                                State::SyncOne
                            }
                            (State::Data { length }, _) => State::Data { length: length - 1 },
                            _ => State::SyncOne,
                        };

                        current_packet.push(curr_byte);
                    }
                    None => match producer.read_from(&mut self.reader, None) {
                        Ok(_) => (),
                        Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => (),
                        Err(e) => eprintln!("{:?}", e),
                    },
                }
            }

            if !current_packet.is_empty() {
                let packet = match Packet::from_bytes(&current_packet) {
                    Ok(packet) => packet,
                    Err(_) => {
                        eprintln!("Failed");
                        continue;
                    }
                };

                (self.handler)(packet);
            }
        }
    }
}
