use derive_field::FieldExtract;
use packet::{Field, RawField};
use anyhow::Result;
use std::convert::TryInto;
use std::boxed::Box;


#[derive(FieldExtract)]
#[descriptor(0x80, 0x04)]
struct ScaledAccelerometerVector {
    x: f32,
    y: f32,
    z: f32
}

#[derive(FieldExtract)]
#[descriptor(0x80, 0x05)]
struct ScaledGyroVector {
    x: f32,
    y: f32,
    z: f32
}

#[derive(FieldExtract)]
#[descriptor(0x80, 0x06)]
struct ScaledMagnetometerVector {
    x: f32,
    y: f32,
    z: f32
}
