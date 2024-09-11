/*----------------------------------------------------------------------------------------------------------
 *  Copyright (c) Peter Bjorklund. All rights reserved. https://github.com/conclave-rust/room-session
 *  Licensed under the MIT License. See LICENSE in the project root for license information.
 *--------------------------------------------------------------------------------------------------------*/
//! The Conclave Room Protocol Serialization

use std::io::{Error, ErrorKind, Result};

use conclave_types::{ConnectionToLeader, Knowledge, Term};
use flood_rs::{ReadOctetStream, WriteOctetStream};
use conclave_room_session::ConnectionIndex;

use crate::ClientReceiveCommand::RoomInfoType;
use crate::ServerReceiveCommand::PingCommandType;

/// Sent from Client to Server


#[derive(Debug, PartialEq)]
pub struct PingCommand {
    pub term: Term,
    pub knowledge: Knowledge,
    pub has_connection_to_leader: ConnectionToLeader,
}

impl PingCommand {
    pub fn to_octets(&self, stream: &mut dyn WriteOctetStream) -> Result<()> {
        stream.write_u16(self.term.0)?;
        stream.write_u64(self.knowledge.0)?;
        stream.write_u8(self.has_connection_to_leader.to_u8())?;

        Ok(())
    }

    pub fn from_cursor(stream: &mut dyn ReadOctetStream) -> Result<Self> {
        Ok(Self {
            term: Term(stream.read_u16()?),
            knowledge: Knowledge(stream.read_u64()?),
            has_connection_to_leader: ConnectionToLeader::from_u8(stream.read_u8()?).ok_or(Error::new(ErrorKind::InvalidData, "Option is None"))?,
        })
    }
}

/// Sent from Server to Client
#[derive(Debug, PartialEq)]
pub struct RoomInfoCommand {
    pub term: Term,
    pub leader_index: ConnectionIndex,
    pub my_index: ConnectionIndex,
    /// An id or address that can be used to reach out to the leader (via a relay server for instance)
    pub leader_id: String,
}

impl RoomInfoCommand {
    pub fn to_octets(&self, stream: &mut dyn WriteOctetStream) -> Result<()> {
        stream.write_u16(self.term.0)?;
        stream.write_u16(self.leader_index.0)?;
        stream.write_u16(self.my_index.0)?;
        stream.write_u8(self.leader_id.len() as u8)?;
        stream.write(self.leader_id.as_bytes())?;

        Ok(())
    }

    pub fn from_cursor(stream: &mut dyn ReadOctetStream) -> Result<Self> {
        let term = Term(stream.read_u16()?);
        let leader_index = ConnectionIndex(stream.read_u16()?);
        let my_index = ConnectionIndex(stream.read_u16()?);
        let leader_id_length = stream.read_u8()? as usize;
        let mut leader_id = String::with_capacity(dbg!(leader_id_length));

        stream.read(unsafe {dbg!(leader_id.as_bytes_mut())})?;

        Ok(Self {
            term,
            leader_index,
            my_index,
            leader_id,
        })
    }
}

#[derive(Debug)]
pub enum ServerReceiveCommand {
    PingCommandType(PingCommand),
}

impl ServerReceiveCommand {
    pub fn to_octets(&self, stream: &mut impl WriteOctetStream) -> Result<()> {
        let command_type_id = match self {
            PingCommandType(_) => PING_COMMAND_TYPE_ID,
            // _ => return Err(format!("unsupported command {:?}", self)),
        };

        stream.write_u8(command_type_id)?;

        match self {
            PingCommandType(ping_command) => {
                ping_command.to_octets(stream)?;
            } // _ => return Err(format!("unknown command enum {:?}", self)),
        }

        Ok(())
    }

    pub fn from_stream(stream: &mut dyn ReadOctetStream) -> Result<ServerReceiveCommand> {
        let command_type_id = stream.read_u8()?;
        match command_type_id {
            PING_COMMAND_TYPE_ID => Ok(PingCommandType(PingCommand::from_cursor(stream)?)),
            _ => Err(Error::new(
                ErrorKind::Other,
                format!("unknown command 0x{:x}", command_type_id),
            )),
        }
    }
}

pub const PING_COMMAND_TYPE_ID: u8 = 0x0a;
pub const ROOM_INFO_COMMAND_TYPE_ID: u8 = 0x2a; // Ping Response

#[derive(Debug)]
pub enum ClientReceiveCommand {
    RoomInfoType(RoomInfoCommand),
}

impl ClientReceiveCommand {
    pub fn to_octets(&self, stream: &mut dyn WriteOctetStream) -> Result<()> {
        let command_type_id = match self {
            RoomInfoType(_) => ROOM_INFO_COMMAND_TYPE_ID,
            // _ => return Err(format!("unsupported command {:?}", self)),
        };

        stream.write_u8(command_type_id)?;

        match self {
            RoomInfoType(room_info_command) => room_info_command.to_octets(stream)?, // _ => return Err(format!("unknown command enum {:?}", self)),
        }

        Ok(())
    }

    pub fn from_octets(stream: &mut impl ReadOctetStream) -> Result<ClientReceiveCommand> {
        let command_type_id = stream.read_u8()?;
        match command_type_id {
            ROOM_INFO_COMMAND_TYPE_ID => Ok(RoomInfoType(RoomInfoCommand::from_cursor(stream)?)),
            _ => Err(Error::new(
                ErrorKind::Other,
                format!("unknown command 0x{:x}", command_type_id),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use conclave_types::{Knowledge, Term, ConnectionToLeader};
    use flood_rs::{InOctetStream, OutOctetStream};
    use conclave_room_session::ConnectionIndex;

    use crate::ClientReceiveCommand::RoomInfoType;
    use crate::ServerReceiveCommand::PingCommandType;
    use crate::{
        ClientReceiveCommand, PingCommand, ServerReceiveCommand, PING_COMMAND_TYPE_ID, ROOM_INFO_COMMAND_TYPE_ID
    };

    #[test]
    fn check_serializer() {
        let ping_command = PingCommand {
            term: Term(32),
            knowledge: Knowledge(444441),
            has_connection_to_leader: ConnectionToLeader::Unknown,
        };

        let mut out_stream = OutOctetStream::new();
        ping_command.to_octets(&mut out_stream).unwrap();

        let mut in_stream = InOctetStream::new(out_stream.data);
        let in_stream_ref = &mut in_stream;
        let deserialized_ping_command = PingCommand::from_cursor(in_stream_ref).unwrap();

        println!("before {:?}", &ping_command);
        println!("after {:?}", &deserialized_ping_command);
        assert_eq!(ping_command, deserialized_ping_command);
    }

    #[test]
    fn check_server_receive_message() {
        const EXPECTED_KNOWLEDGE_VALUE: u64 = 17718865395771014920;

        let octets = [
            PING_COMMAND_TYPE_ID,
            0x00,
            0x20, // Term
            0xF5,
            0xE6,
            0x0E,
            0x32,
            0xE9,
            0xE4,
            0x7F,
            0x08, // Knowledge
            0x01, // Has Connection
        ];

        let mut in_stream = InOctetStream::new(Vec::from(octets));

        let message = ServerReceiveCommand::from_stream(&mut in_stream).unwrap();

        match message {
            PingCommandType(ping_command) => {
                println!("received {:?}", &ping_command);
                assert_eq!(ping_command.term.0, 0x20);
                assert_eq!(ping_command.knowledge.0, EXPECTED_KNOWLEDGE_VALUE);
                assert_eq!(ping_command.has_connection_to_leader, ConnectionToLeader::Connected);
            } // _ => assert!(false, "should be ping command"),
        }
    }

    #[test]
    fn check_client_receive_message() {
        const EXPECTED_LEADER_INDEX: u16 = 1;
        const EXPECTED_MY_INDEX: u16 = 2;
        const EXPECTED_LEADER_ID: u8 = 'A' as u8;

        let octets = [
            ROOM_INFO_COMMAND_TYPE_ID,
            0x00,                        // Term
            0x4A,                        // Term (lower)
            0x00,                        // Leader index
            EXPECTED_LEADER_INDEX as u8, // Leader index (lower)
            0x00,                        // My index
            EXPECTED_MY_INDEX as u8,     // My index (lower)
            0x01,                        // Leader id length
            EXPECTED_LEADER_ID,          // Leader id
        ];

        let mut in_stream = InOctetStream::new(Vec::from(octets));

        let message = ClientReceiveCommand::from_octets(&mut in_stream).unwrap();

        match message {
            RoomInfoType(room_info) => {
                println!("received {:?}", &room_info);
                assert_eq!(room_info.term.0, 0x4A);
                assert_eq!(room_info.leader_index.0, EXPECTED_LEADER_INDEX);
                assert_eq!(room_info.my_index.0, EXPECTED_MY_INDEX);
                assert_eq!(room_info.leader_id.len(), 1);
                assert_eq!(room_info.leader_id.as_bytes()[0], EXPECTED_LEADER_ID);
            } // _ => assert!(false, "should be room info command"),
        }
    }
}
