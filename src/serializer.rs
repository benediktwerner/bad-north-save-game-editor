use std::collections::{HashMap, HashSet, VecDeque};

use byteorder::{LittleEndian, WriteBytesExt};

use super::records::*;

pub fn serialize(rec: &DeserializedRecord) -> Vec<u8> {
    Serializer::new().serialize(rec)
}

struct Serializer {
    output: Vec<u8>,
    todo: VecDeque<i32>,
    done: HashSet<i32>,
    class_metadata: HashMap<usize, i32>,
}

impl Serializer {
    fn new() -> Self {
        Self {
            output: Vec::with_capacity(0x1000),
            todo: VecDeque::new(),
            done: HashSet::new(),
            class_metadata: HashMap::new(),
        }
    }

    fn add_todo(&mut self, id: i32) {
        if !self.done.contains(&id) {
            self.done.insert(id);
            self.todo.push_back(id);
        }
    }

    fn serialize(mut self, rec: &DeserializedRecord) -> Vec<u8> {
        self.write_u8(0);
        self.write_i32(rec.root_id);
        self.write_i32(rec.header_id);
        self.write_i32(1);
        self.write_i32(0);

        for (id, record) in &rec.records {
            if let Record::BinaryLibrary(_) = record {
                self.add_todo(*id);
            }
        }

        self.add_todo(rec.root_id);

        while let Some(id) = self.todo.pop_front() {
            self.write_record(rec, id, &rec.records[&id]);
        }

        self.write_u8(11);

        self.output
    }

    fn write_record(&mut self, recs: &DeserializedRecord, id: i32, record: &Record) {
        match record {
            Record::BinaryLibrary(name) => {
                self.write_u8(12);
                self.write_i32(id);
                self.write_string(name);
            }
            Record::Class(class) => {
                let class_type = recs.class_type(class);
                if class_type.system_class {
                    self.write_u8(4);
                    self.write_class_type(id, class_type);
                } else if let Some(class_id) = self.class_metadata.get(&class.class_type_id) {
                    let class_id = *class_id;
                    self.write_u8(1);
                    self.write_i32(id);
                    self.write_i32(class_id);
                } else {
                    self.write_u8(5);
                    self.write_class_type(id, class_type);
                    self.write_i32(class_type.library_id);
                    self.class_metadata.insert(class.class_type_id, id);
                }
                for (member, member_type) in
                    class.members.iter().zip(class_type.member_types.iter())
                {
                    self.write_member(member, member_type);
                }
            }
            // Record::ObjectArray(vals) => {
            //     self.write_u8(16);
            //     self.write_i32(id);
            //     self.write_i32(vals.len() as i32);
            //     for val in vals {
            //         self.write_member(val, &MemberType::Object);
            //     }
            // }
            Record::BinaryArray(typ, vals) => {
                self.write_u8(7);
                self.write_i32(id);
                self.write_u8(0);
                self.write_i32(1);
                self.write_i32(vals.len() as i32);
                self.write_member_type(typ);
                self.write_member_type_additional_info(typ);
                for val in vals {
                    self.write_member(val, typ);
                }
            }
            Record::PrimitiveArray(typ, vals) => {
                self.write_u8(15);
                self.write_i32(id);
                self.write_i32(vals.len() as i32);
                self.write_primitive_type(typ);
                for val in vals {
                    self.write_primitive(val);
                }
            }
            Record::String(val) => {
                self.write_u8(6);
                self.write_i32(id);
                self.write_string(val);
            }
        }
    }

    fn write_class_type(&mut self, id: i32, class_type: &ClassType) {
        self.write_i32(id);
        self.write_string(&class_type.name);
        self.write_i32(class_type.member_names.len() as i32);

        for name in &class_type.member_names {
            self.write_string(name);
        }

        for t in &class_type.member_types {
            self.write_member_type(t);
        }

        for t in &class_type.member_types {
            self.write_member_type_additional_info(t);
        }
    }

    fn write_member_type(&mut self, typ: &MemberType) {
        match typ {
            MemberType::Primitive(_) => self.write_u8(0),
            MemberType::String => self.write_u8(1),
            MemberType::Object => self.write_u8(2),
            MemberType::SystemClass(_) => self.write_u8(3),
            MemberType::Class(..) => self.write_u8(4),
            MemberType::ObjectArray => self.write_u8(5),
            MemberType::StringArray => self.write_u8(6),
            MemberType::PrimitiveArray(_) => self.write_u8(7),
        }
    }

    fn write_member_type_additional_info(&mut self, typ: &MemberType) {
        match typ {
            MemberType::Primitive(t) => self.write_primitive_type(t),
            MemberType::SystemClass(name) => self.write_string(name),
            MemberType::Class(s, i) => {
                self.write_string(s);
                self.write_i32(*i);
            }
            MemberType::PrimitiveArray(t) => self.write_primitive_type(t),
            _ => (),
        }
    }

    fn write_member(&mut self, member: &Member, t: &MemberType) {
        if let MemberType::Primitive(_) = t {
            if let Member::Primitive(val) = member {
                self.write_primitive(val);
            } else {
                panic!("Non primitive member for primitive field");
            }
        } else {
            match member {
                Member::Primitive(val) => {
                    self.write_u8(8);
                    self.write_primitive_type(&val.primitive_type());
                    self.write_primitive(val);
                }
                Member::Reference(id) => {
                    self.write_u8(9);
                    self.write_i32(*id);
                    self.add_todo(*id);
                }
                Member::Null => self.write_u8(10),
                Member::NullMultiple(count) => {
                    if *count < 0x100 {
                        self.write_u8(13);
                        self.write_u8(*count as u8);
                    } else {
                        self.write_u8(14);
                        self.write_i32(*count);
                    }
                }
            }
        }
    }

    fn write_primitive_type(&mut self, typ: &PrimitiveType) {
        self.write_u8(match typ {
            PrimitiveType::Boolean => 1,
            PrimitiveType::Byte => 2,
            PrimitiveType::Char => 3,
            PrimitiveType::Decimal => 5,
            PrimitiveType::Double => 6,
            PrimitiveType::Int16 => 7,
            PrimitiveType::Int32 => 8,
            PrimitiveType::Int64 => 9,
            PrimitiveType::Int8 => 10,
            PrimitiveType::Single => 11,
            PrimitiveType::TimeSpan => 12,
            PrimitiveType::DateTime => 13,
            PrimitiveType::UInt16 => 14,
            PrimitiveType::UInt32 => 15,
            PrimitiveType::UInt64 => 16,
            PrimitiveType::Null => 17,
            PrimitiveType::String => 18,
        });
    }

    fn write_primitive(&mut self, val: &Primitive) {
        match val {
            Primitive::Boolean(val) => self.write_u8(*val as u8),
            Primitive::Byte(val) => self.write_u8(*val),
            Primitive::Char(_val) => todo!(),
            Primitive::Decimal(val) => self.write_string(val),
            Primitive::Double(val) => self.output.write_f64::<LittleEndian>(*val).unwrap(),
            Primitive::Int16(val) => self.output.write_i16::<LittleEndian>(*val).unwrap(),
            Primitive::Int32(val) => self.output.write_i32::<LittleEndian>(*val).unwrap(),
            Primitive::Int64(val) => self.output.write_i64::<LittleEndian>(*val).unwrap(),
            Primitive::Int8(val) => self.output.write_i8(*val).unwrap(),
            Primitive::Single(val) => self.output.write_f32::<LittleEndian>(*val).unwrap(),
            Primitive::TimeSpan(val) => self.output.write_i64::<LittleEndian>(*val).unwrap(),
            Primitive::DateTime(val) => self.output.write_i64::<LittleEndian>(*val).unwrap(),
            Primitive::UInt16(val) => self.output.write_u16::<LittleEndian>(*val).unwrap(),
            Primitive::UInt32(val) => self.output.write_u32::<LittleEndian>(*val).unwrap(),
            Primitive::UInt64(val) => self.output.write_u64::<LittleEndian>(*val).unwrap(),
            Primitive::Null => (),
            Primitive::String(val) => self.write_string(val),
        }
    }

    fn write_string(&mut self, val: &str) {
        let mut length = val.len();
        assert!(length <= 0x7FFF_FFFF);
        loop {
            let val = (length & 0b111_1111) as u8;
            length >>= 7;
            if length == 0 {
                self.write_u8(val);
                break;
            }
            self.write_u8(val | 0b1000_0000);
        }
        self.output.extend(val.as_bytes());
    }

    fn write_u8(&mut self, i: u8) {
        self.output.write_u8(i).unwrap();
    }

    fn write_i32(&mut self, i: i32) {
        self.output.write_i32::<LittleEndian>(i).unwrap();
    }
}
