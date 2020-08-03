use std::collections::HashMap;
use std::io::{Error, ErrorKind};

use byteorder::{LittleEndian, ReadBytesExt};

use super::records::*;

type Result<T = ()> = std::io::Result<T>;

const MESSAGE_END: u8 = 11;

pub fn parse(bytes: &[u8]) -> Result<DeserializedRecord> {
    Parser::new(bytes).parse()
}

struct Parser<'a> {
    bytes: &'a [u8],
    records: HashMap<i32, Record>,
    class_types: Vec<ClassType>,
    class_metadata: HashMap<i32, usize>,
}

impl<'a> Parser<'a> {
    fn new(bytes: &'a [u8]) -> Parser<'a> {
        Self {
            bytes,
            records: HashMap::new(),
            class_types: Vec::new(),
            class_metadata: HashMap::new(),
        }
    }

    fn parse(mut self) -> Result<DeserializedRecord> {
        let header_magic = self.parse_u8()?;
        if header_magic != 0 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("Header byte must be 0 but was {}", header_magic),
            ));
        }

        let root_id = self.parse_i32()?;
        let header_id = self.parse_i32()?;
        let major_version = self.parse_i32()?;
        let minor_version = self.parse_i32()?;
        assert!(
            major_version == 1,
            "unsupported major version (!= 1): {}",
            major_version
        );
        assert!(
            minor_version == 0,
            "unsupported minor version (!= 0): {}",
            minor_version
        );

        while self.peek_byte()? != MESSAGE_END {
            let (id, record) = self.parse_record()?;
            self.add_record(id, record)?;
        }

        Ok(DeserializedRecord {
            root_id,
            header_id,
            records: self.records,
            class_types: self.class_types,
        })
    }

    fn add_record(&mut self, id: i32, record: Record) -> Result {
        if self.records.insert(id, record).is_some() {
            Err(Error::new(
                ErrorKind::AlreadyExists,
                format!("Duplicate record with id {}", id),
            ))
        } else {
            Ok(())
        }
    }

    fn parse_record(&mut self) -> Result<(i32, Record)> {
        match self.parse_u8()? {
            1 => self.parse_class_with_id(),
            4 => self.parse_system_class_with_members_and_type(),
            5 => self.parse_class_with_members_and_type(),
            6 => self.parse_binary_object_string(),
            7 => self.parse_binary_array(),
            12 => self.parse_binary_library(),
            15 => self.parse_array_single_primitive(),
            other => Err(Error::new(
                ErrorKind::Other,
                format!("Unknown record type: {}", other),
            )),
        }
    }

    fn parse_binary_library(&mut self) -> Result<(i32, Record)> {
        let id = self.parse_i32()?;
        let name = self.parse_string()?;
        Ok((id, Record::BinaryLibrary(name)))
    }

    fn parse_binary_object_string(&mut self) -> Result<(i32, Record)> {
        let id = self.parse_i32()?;
        let val = self.parse_string()?;
        Ok((id, Record::String(val)))
    }

    fn parse_binary_array(&mut self) -> Result<(i32, Record)> {
        let id = self.parse_i32()?;
        let array_type = self.parse_u8()?;
        if array_type != 0 {
            todo!("Array with BinaryArrayType {}", array_type);
        }
        let rank = self.parse_i32()?;
        if rank != 1 {
            todo!("Array with rank {}", rank);
        }
        let length = self.parse_i32()? as usize;
        let member_type = self.parse_u8()?;
        let member_type = self.parse_member_type(member_type)?;
        let mut vals = Vec::with_capacity(length);
        while vals.len() < length {
            match self.parse_member(&member_type)? {
                Member::NullMultiple(count) => {
                    for _ in 0..count as usize {
                        vals.push(Member::Null);
                    }
                }
                other => vals.push(other),
            }
        }
        Ok((id, Record::BinaryArray(member_type, vals)))
    }

    fn parse_array_single_primitive(&mut self) -> Result<(i32, Record)> {
        let id = self.parse_i32()?;
        let length = self.parse_i32()? as usize;
        let typ = self.parse_primitive_type()?;
        let mut vals = Vec::with_capacity(length);
        for _ in 0..length {
            vals.push(self.parse_primitive(&typ)?);
        }
        Ok((id, Record::PrimitiveArray(typ, vals)))
    }

    fn parse_class_with_id(&mut self) -> Result<(i32, Record)> {
        let id = self.parse_i32()?;
        let metadata_id = self.parse_i32()?;
        let class_type_id = self.class_metadata[&metadata_id];
        let class_type = &self.class_types[class_type_id];
        let member_types = class_type.member_types.clone();
        let class = Class {
            class_type_id,
            members: self.parse_members(&member_types)?,
        };
        Ok((id, Record::Class(class)))
    }

    fn parse_system_class_with_members_and_type(&mut self) -> Result<(i32, Record)> {
        let (id, name, member_names) = self.parse_class_info()?;
        let member_types = self.parse_member_types(member_names.len())?;
        let members = self.parse_members(&member_types)?;

        let class = Class {
            class_type_id: self.class_types.len(),
            members,
        };
        self.class_metadata.insert(id, class.class_type_id);

        self.class_types.push(ClassType {
            name,
            library_id: 0,
            system_class: true,
            member_names,
            member_types,
        });

        Ok((id, Record::Class(class)))
    }

    fn parse_class_with_members_and_type(&mut self) -> Result<(i32, Record)> {
        let (id, name, member_names) = self.parse_class_info()?;
        let member_types = self.parse_member_types(member_names.len())?;
        let library_id = self.parse_i32()?;
        let members = self.parse_members(&member_types)?;

        let class = Class {
            class_type_id: self.class_types.len(),
            members,
        };
        self.class_metadata.insert(id, class.class_type_id);

        self.class_types.push(ClassType {
            name,
            library_id,
            system_class: false,
            member_names,
            member_types,
        });

        Ok((id, Record::Class(class)))
    }

    fn parse_members(&mut self, types: &[MemberType]) -> Result<Vec<Member>> {
        let mut result = Vec::with_capacity(types.len());
        let mut i = 0;
        while i < types.len() {
            match self.parse_member(&types[i])? {
                Member::NullMultiple(count) => {
                    for _ in 0..count as usize {
                        result.push(Member::Null);
                    }
                }
                other => result.push(other),
            }
            i += 1;
        }
        Ok(result)
    }

    fn parse_member(&mut self, typ: &MemberType) -> Result<Member> {
        if let MemberType::Primitive(prim_typ) = typ {
            return Ok(Member::Primitive(self.parse_primitive(prim_typ)?));
        }
        match self.parse_u8()? {
            1 => {
                let (id, record) = self.parse_class_with_id()?;
                self.add_record(id, record)?;
                Ok(Member::Reference(id))
            }
            4 => {
                let (id, record) = self.parse_system_class_with_members_and_type()?;
                self.add_record(id, record)?;
                Ok(Member::Reference(id))
            }
            5 => {
                let (id, record) = self.parse_class_with_members_and_type()?;
                self.add_record(id, record)?;
                Ok(Member::Reference(id))
            }
            15 => {
                let (id, record) = self.parse_array_single_primitive()?;
                self.add_record(id, record)?;
                Ok(Member::Reference(id))
            }
            6 => {
                let (id, record) = self.parse_binary_object_string()?;
                self.add_record(id, record)?;
                Ok(Member::Reference(id))
            }
            7 => {
                let (id, record) = self.parse_binary_array()?;
                self.add_record(id, record)?;
                Ok(Member::Reference(id))
            }
            9 => Ok(Member::Reference(self.parse_i32()?)),
            10 => Ok(Member::Null),
            14 => Ok(Member::NullMultiple(self.parse_i32()?)),
            13 => Ok(Member::NullMultiple(self.parse_u8()? as i32)),
            other => Err(Error::new(
                ErrorKind::InvalidData,
                format!("Unexpected record type for member: {}", other),
            )),
        }
    }

    fn parse_primitive(&mut self, typ: &PrimitiveType) -> Result<Primitive> {
        Ok(match typ {
            PrimitiveType::Boolean => Primitive::Boolean(self.parse_u8()? != 0),
            PrimitiveType::Byte => Primitive::Byte(self.parse_u8()?),
            PrimitiveType::Char => Primitive::Char(todo!("primitive char")),
            PrimitiveType::Decimal => Primitive::Decimal(self.parse_string()?),
            PrimitiveType::Double => Primitive::Double(self.bytes.read_f64::<LittleEndian>()?),
            PrimitiveType::Int16 => Primitive::Int16(self.bytes.read_i16::<LittleEndian>()?),
            PrimitiveType::Int32 => Primitive::Int32(self.bytes.read_i32::<LittleEndian>()?),
            PrimitiveType::Int64 => Primitive::Int64(self.bytes.read_i64::<LittleEndian>()?),
            PrimitiveType::Int8 => Primitive::Int8(self.bytes.read_i8()?),
            PrimitiveType::Single => Primitive::Single(self.bytes.read_f32::<LittleEndian>()?),
            PrimitiveType::TimeSpan => Primitive::TimeSpan(self.bytes.read_i64::<LittleEndian>()?),
            PrimitiveType::DateTime => Primitive::DateTime(self.bytes.read_i64::<LittleEndian>()?),
            PrimitiveType::UInt16 => Primitive::UInt16(self.bytes.read_u16::<LittleEndian>()?),
            PrimitiveType::UInt32 => Primitive::UInt32(self.bytes.read_u32::<LittleEndian>()?),
            PrimitiveType::UInt64 => Primitive::UInt64(self.bytes.read_u64::<LittleEndian>()?),
            PrimitiveType::Null => Primitive::Null,
            PrimitiveType::String => Primitive::String(self.parse_string()?),
        })
    }

    fn parse_primitive_type(&mut self) -> Result<PrimitiveType> {
        Ok(match self.parse_u8()? {
            1 => PrimitiveType::Boolean,
            2 => PrimitiveType::Byte,
            3 => PrimitiveType::Char,
            // 4 unused
            5 => PrimitiveType::Decimal,
            6 => PrimitiveType::Double,
            7 => PrimitiveType::Int16,
            8 => PrimitiveType::Int32,
            9 => PrimitiveType::Int64,
            10 => PrimitiveType::Int8,
            11 => PrimitiveType::Single,
            12 => PrimitiveType::TimeSpan,
            13 => PrimitiveType::DateTime,
            14 => PrimitiveType::UInt16,
            15 => PrimitiveType::UInt32,
            16 => PrimitiveType::UInt64,
            17 => PrimitiveType::Null,
            18 => PrimitiveType::String,
            other => {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!("Unexpected primitive type: {}", other),
                ))
            }
        })
    }

    fn parse_member_types(&mut self, count: usize) -> Result<Vec<MemberType>> {
        let mut result = Vec::with_capacity(count);
        for &t in self.take_bytes(count)? {
            result.push(self.parse_member_type(t)?);
        }
        Ok(result)
    }

    fn parse_member_type(&mut self, typ: u8) -> Result<MemberType> {
        Ok(match typ {
            0 => MemberType::Primitive(self.parse_primitive_type()?),
            1 => MemberType::String,
            2 => MemberType::Object,
            3 => MemberType::SystemClass(self.parse_string()?),
            4 => MemberType::Class(self.parse_string()?, self.parse_i32()?),
            5 => MemberType::ObjectArray,
            6 => MemberType::StringArray,
            7 => MemberType::PrimitiveArray(self.parse_primitive_type()?),
            other => {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!("Unexpected member type: {}", other),
                ))
            }
        })
    }

    fn parse_class_info(&mut self) -> Result<(i32, String, Vec<String>)> {
        let id = self.parse_i32()?;
        let name = self.parse_string()?;
        let member_count = self.parse_i32()? as usize;
        let mut members = Vec::with_capacity(member_count);
        for _ in 0..member_count {
            members.push(self.parse_string()?);
        }
        Ok((id, name, members))
    }

    fn parse_string(&mut self) -> Result<String> {
        let length = self.parse_length()?;
        let bytes = self.take_bytes(length as usize)?;
        Ok(std::str::from_utf8(bytes).unwrap().into())
    }

    fn parse_length(&mut self) -> Result<u32> {
        let mut length = 0;
        for bit_range in 0..5 {
            let byte = self.parse_u8()? as u32;
            length += (byte & 0b111_1111) << (bit_range * 7);
            if byte & 0b1000_0000 == 0 {
                break;
            }
        }
        Ok(length)
    }

    fn peek_byte(&self) -> Result<u8> {
        match self.bytes.get(0) {
            Some(x) => Ok(*x),
            None => Err(ErrorKind::UnexpectedEof.into()),
        }
    }

    fn take_bytes<'b>(&'b mut self, n: usize) -> Result<&'a [u8]> {
        if self.bytes.len() < n {
            return Err(ErrorKind::UnexpectedEof.into());
        }

        let result = &self.bytes[..n];
        self.bytes = &self.bytes[n..];
        Ok(result)
    }

    fn parse_u8(&mut self) -> Result<u8> {
        self.bytes.read_u8()
    }

    fn parse_i32(&mut self) -> Result<i32> {
        self.bytes.read_i32::<LittleEndian>()
    }
}
