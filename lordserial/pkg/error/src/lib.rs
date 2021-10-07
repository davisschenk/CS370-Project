use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParsingError {
    #[error("Not enough bytes while parsing. Got {provided}, but expected {required}")]
    SrcInsufficent {required: usize, provided: usize},

    #[error("Packet invalid, bad checksum.")]
    BadChecksum
}
