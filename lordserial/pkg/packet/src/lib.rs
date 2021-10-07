use anyhow::Result;
use error::ParsingError;
use desert::FromBytesBE;

pub trait PacketComponent {
    fn to_bytes(&self) -> Result<Vec<u8>>;
    fn from_bytes(bytes: &[u8]) -> Result<Self> where Self: Sized;
}

#[derive(Debug)]
pub struct Header {
    pub sync_one: u8,
    pub sync_two: u8,
    pub descriptor: u8,
}

impl PacketComponent for Header {
    fn to_bytes(&self) -> Result<Vec<u8>> {
        Ok(vec![self.sync_one, self.sync_two, self.descriptor])
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 3 {
            return Err(ParsingError::SrcInsufficent {
                required: 3,
                provided: bytes.len(),
            }.into());
        }

        Ok(Self {
            sync_one: bytes[0],
            sync_two: bytes[1],
            descriptor: bytes[2],
        })
    }
}

pub trait Field<T: std::convert::TryFrom<RawField, Error = anyhow::Error> = Self> {
    const DATA_DESCRIPTOR: u8;
    const SET_DESCRIPTOR: u8;
}

#[derive(Debug)]
pub struct RawField {
    pub length: u8,
    pub descriptor: u8,
    pub data: Vec<u8>,
}

impl PacketComponent for RawField {
    fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut v = vec![self.length, self.descriptor];
        
        for i in &self.data {
            v.push(*i);
        }
        
        Ok(v)
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let length = bytes[0] as usize;
        let descriptor = bytes[1];
        let data = bytes[2..].to_vec();

        if bytes.len() != length {
            return Err(ParsingError::SrcInsufficent {
                required: length,
                provided: bytes.len(),
            }.into());
        }

        Ok(Self {
            length: bytes[0],
            descriptor,
            data,
        })
    }
}

impl RawField {    
    pub fn extract<T: FromBytesBE>(&self, offset: usize) -> Result<T> {
        let (_, num) = T::from_bytes_be(&self.data[offset..]).map_err(|_| ParsingError::BadChecksum)?;
        Ok(num)
    }
}

#[derive(Debug)]
pub struct Payload {
    pub length: u8,
    pub fields: Vec<RawField>,
}

impl PacketComponent for Payload {
    fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut bytes: Vec<u8> = vec![self.length];

        for field in &self.fields {
            bytes.append(&mut field.to_bytes()?);
        }

        Ok(bytes)
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let length = bytes[0] as usize;
        let mut offset: usize = 1;
        let mut fields: Vec<RawField> = Vec::new();

        while offset < length {
            let field_length = bytes[offset] as usize;
            let field = RawField::from_bytes(&bytes[offset..(offset+field_length)])?;

            fields.push(field);
            offset += field_length;
        }

        Ok(Self {
            length: bytes[0],
            fields
        })

    }
}


#[derive(Debug)]
pub struct Checksum {
    pub msb: u8,
    pub lsb: u8,
}

impl PacketComponent for Checksum {
    fn to_bytes(&self) -> Result<Vec<u8>> {
        return Ok(vec![self.msb, self.lsb]);
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 2 {
            return Err(ParsingError::SrcInsufficent {
                required: 2,
                provided: bytes.len(),
            }.into());
        }

        Ok(Self {
            msb: bytes[0],
            lsb: bytes[1],
        })
    }
}

impl Checksum {
    fn validate_bytes(&self, bytes: &[u8]) -> bool {
        let mut byte_one: u8 = 0;
        let mut byte_two: u8 = 0;

        for byte in &bytes[..bytes.len() - 2] {
            byte_one = byte_one.wrapping_add(*byte);
            byte_two = byte_two.wrapping_add(byte_one);
        }

        byte_one == self.msb && byte_two == self.lsb
    }
}

#[derive(Debug)]
pub struct Packet {
    pub header: Header,
    pub payload: Payload,
    pub checksum: Checksum
}

impl PacketComponent for Packet {
    fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut bytes: Vec<u8> = vec![];

        bytes.append(&mut self.header.to_bytes()?);
        bytes.append(&mut self.payload.to_bytes()?);
        bytes.append(&mut self.checksum.to_bytes()?);

        Ok(bytes)

    }

    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let checksum = Checksum::from_bytes(&bytes[bytes.len() - 2..])?;

        if !checksum.validate_bytes(&bytes) {
            return Err(ParsingError::BadChecksum.into());
        }


        let header = Header::from_bytes(&bytes[..3])?;
        let payload = Payload::from_bytes(&bytes[3..bytes.len() - 2])?;

        Ok(Self {
            header,
            payload,
            checksum
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Packet, PacketComponent};

    #[test]
    fn test_packet() {
        let v = vec![0x75, 0x65, 0x80, 0x5E, 0x0E, 0x12, 0x40, 0x67, 0xD2, 0x7E, 0xF9, 0xDB, 0x22, 0xD1, 0x00, 0x00, 0x00, 0x06, 0x12, 0x0A, 0x3C, 0xB5, 0x86, 0xAA, 0x3D, 0xBE, 0xB0, 0x7E, 0x3F, 0x7E, 0xD0, 0x90, 0x3C, 0x10, 0xE8, 0xAB, 0x0E, 0x0C, 0x40, 0x47, 0xAB, 0x6C, 0x3D, 0x2D, 0xFD, 0xDC, 0x40, 0x3D, 0x17, 0xF4, 0x0E, 0x04, 0x3D, 0x36, 0xFC, 0xEA, 0xBC, 0xBE, 0x8D, 0xC0, 0x3F, 0x7F, 0x96, 0xDC, 0x0E, 0x05, 0x3A, 0x0A, 0x45, 0x73, 0x3A, 0xFB, 0x74, 0x4F, 0x3A, 0x6E, 0x7B, 0x95, 0x0E, 0x06, 0xBE, 0xD5, 0x4B, 0x19, 0x3D, 0x9D, 0x18, 0xC7, 0xBB, 0xE2, 0xCB, 0xE8, 0x06, 0x17, 0x44, 0x53, 0x1B, 0xB8, 0x3D, 0x55];
        
        assert_eq!(v, Packet::from_bytes(&v).unwrap().to_bytes().unwrap());
    }
}