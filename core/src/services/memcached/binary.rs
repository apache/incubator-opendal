// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::io::{self};
use tokio::net::TcpStream;
use tokio_native_tls::TlsStream;

use crate::raw::*;
use crate::*;

pub(super) mod constants {
    pub const OK_STATUS: u16 = 0x0;
    pub const KEY_NOT_FOUND: u16 = 0x1;
}

pub enum Opcode {
    Get = 0x00,
    Set = 0x01,
    Delete = 0x04,
    Version = 0x0b,
    StartAuth = 0x21,
}

pub enum Magic {
    Request = 0x80,
}

#[derive(Debug)]
pub struct StoreExtras {
    pub flags: u32,
    pub expiration: u32,
}

#[derive(Debug, Default)]
pub struct PacketHeader {
    pub magic: u8,
    pub opcode: u8,
    pub key_length: u16,
    pub extras_length: u8,
    pub data_type: u8,
    pub vbucket_id_or_status: u16,
    pub total_body_length: u32,
    pub opaque: u32,
    pub cas: u64,
}

impl PacketHeader {
    pub async fn write(self, writer: &mut dyn AsyncWrite) -> io::Result<()> {
        writer.write_u8(self.magic).await?;
        writer.write_u8(self.opcode).await?;
        writer.write_u16(self.key_length).await?;
        writer.write_u8(self.extras_length).await?;
        writer.write_u8(self.data_type).await?;
        writer.write_u16(self.vbucket_id_or_status).await?;
        writer.write_u32(self.total_body_length).await?;
        writer.write_u32(self.opaque).await?;
        writer.write_u64(self.cas).await?;
        Ok(())
    }

    pub async fn read(reader: &mut dyn AsyncRead) -> Result<PacketHeader, io::Error> {
        let header = PacketHeader {
            magic: reader.read_u8().await?,
            opcode: reader.read_u8().await?,
            key_length: reader.read_u16().await?,
            extras_length: reader.read_u8().await?,
            data_type: reader.read_u8().await?,
            vbucket_id_or_status: reader.read_u16().await?,
            total_body_length: reader.read_u32().await?,
            opaque: reader.read_u32().await?,
            cas: reader.read_u64().await?,
        };
        Ok(header)
    }
}

pub struct Response {
    header: PacketHeader,
    _key: Vec<u8>,
    _extras: Vec<u8>,
    value: Vec<u8>,
}

pub struct Connection {
    stream: Box<dyn AsyncRead + AsyncWrite + Send + Unpin>,
}

impl Connection {
    pub async fn new(stream: Box<dyn AsyncRead + AsyncWrite + Send + Unpin>) -> Result<Self, Error> {
        Ok(Self { stream })
    }

    pub async fn auth(&mut self, username: &str, password: &str) -> Result<()> {
        let writer = &mut *self.stream;
        let key = "PLAIN";
        let request_header = PacketHeader {
            magic: Magic::Request as u8,
            opcode: Opcode::StartAuth as u8,
            key_length: key.len() as u16,
            total_body_length: (key.len() + username.len() + password.len() + 2) as u32,
            ..Default::default()
        };
        request_header
            .write(writer)
            .await
            .map_err(new_std_io_error)?;
        writer
            .write_all(key.as_bytes())
            .await
            .map_err(new_std_io_error)?;
        writer
            .write_all(format!("\x00{}\x00{}", username, password).as_bytes())
            .await
            .map_err(new_std_io_error)?;
        writer.flush().await.map_err(new_std_io_error)?;
        parse_response(writer).await?;
        Ok(())
    }

    pub async fn version(&mut self) -> Result<String> {
        let writer = &mut *self.stream;
        let request_header = PacketHeader {
            magic: Magic::Request as u8,
            opcode: Opcode::Version as u8,
            ..Default::default()
        };
        request_header
            .write(writer)
            .await
            .map_err(new_std_io_error)?;
        writer.flush().await.map_err(new_std_io_error)?;
        let response = parse_response(writer).await?;
        let version = String::from_utf8(response.value);
        match version {
            Ok(version) => Ok(version),
            Err(e) => {
                Err(Error::new(ErrorKind::Unexpected, "unexpected data received").set_source(e))
            }
        }
    }

    pub async fn get(&mut self, key: &str) -> Result<Option<Vec<u8>>> {
        let writer = &mut *self.stream;
        let request_header = PacketHeader {
            magic: Magic::Request as u8,
            opcode: Opcode::Get as u8,
            key_length: key.len() as u16,
            total_body_length: key.len() as u32,
            ..Default::default()
        };
        request_header
            .write(writer)
            .await
            .map_err(new_std_io_error)?;
        writer
            .write_all(key.as_bytes())
            .await
            .map_err(new_std_io_error)?;
        writer.flush().await.map_err(new_std_io_error)?;
        match parse_response(writer).await {
            Ok(response) => {
                if response.header.vbucket_id_or_status == 0x1 {
                    return Ok(None);
                }
                Ok(Some(response.value))
            }
            Err(e) => Err(e),
        }
    }

    pub async fn set(&mut self, key: &str, val: &[u8], expiration: u32) -> Result<()> {
        let writer = &mut *self.stream;
        let request_header = PacketHeader {
            magic: Magic::Request as u8,
            opcode: Opcode::Set as u8,
            key_length: key.len() as u16,
            extras_length: 8,
            total_body_length: (8 + key.len() + val.len()) as u32,
            ..Default::default()
        };
        let extras = StoreExtras {
            flags: 0,
            expiration,
        };
        request_header
            .write(writer)
            .await
            .map_err(new_std_io_error)?;
        writer
            .write_u32(extras.flags)
            .await
            .map_err(new_std_io_error)?;
        writer
            .write_u32(extras.expiration)
            .await
            .map_err(new_std_io_error)?;
        writer
            .write_all(key.as_bytes())
            .await
            .map_err(new_std_io_error)?;
        writer.write_all(val).await.map_err(new_std_io_error)?;
        writer.flush().await.map_err(new_std_io_error)?;

        parse_response(writer).await?;
        Ok(())
    }

    pub async fn delete(&mut self, key: &str) -> Result<()> {
        let writer = &mut *self.stream;
        let request_header = PacketHeader {
            magic: Magic::Request as u8,
            opcode: Opcode::Delete as u8,
            key_length: key.len() as u16,
            total_body_length: key.len() as u32,
            ..Default::default()
        };
        request_header
            .write(writer)
            .await
            .map_err(new_std_io_error)?;
        writer
            .write_all(key.as_bytes())
            .await
            .map_err(new_std_io_error)?;
        writer.flush().await.map_err(new_std_io_error)?;
        parse_response(writer).await?;
        Ok(())
    }
}

pub async fn parse_response(reader: &mut dyn AsyncRead) -> Result<Response> {
    let header = PacketHeader::read(reader).await.map_err(new_std_io_error)?;

    if header.vbucket_id_or_status != constants::OK_STATUS
        && header.vbucket_id_or_status != constants::KEY_NOT_FOUND
    {
        return Err(
            Error::new(ErrorKind::Unexpected, "unexpected status received")
                .with_context("message", format!("{}", header.vbucket_id_or_status)),
        );
    }

    let mut extras = vec![0x0; header.extras_length as usize];
    reader
        .read_exact(extras.as_mut_slice())
        .await
        .map_err(new_std_io_error)?;

    let mut key = vec![0x0; header.key_length as usize];
    reader
        .read_exact(key.as_mut_slice())
        .await
        .map_err(new_std_io_error)?;

    let mut value = vec![
        0x0;
        (header.total_body_length - u32::from(header.key_length) - u32::from(header.extras_length))
            as usize
    ];
    reader
        .read_exact(value.as_mut_slice())
        .await
        .map_err(new_std_io_error)?;

    Ok(Response {
        header,
        _key: key,
        _extras: extras,
        value,
    })
}
