use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct DeserializedRecord {
    pub root_id: i32,
    pub header_id: i32,
    pub records: HashMap<i32, Record>,
    pub class_types: Vec<ClassType>,
}

impl DeserializedRecord {
    pub fn class_type(&self, class: &Class) -> &ClassType {
        &self.class_types[class.class_type_id]
    }

    pub fn class_member<'a, 'b>(&'a self, class: &'a Class, name: &'b str) -> &'a Member {
        &class.members[self.class_member_index(class, name)]
    }

    pub fn class_member_deref<'a, 'b>(&'a self, class: &'a Class, name: &'b str) -> &'a Record {
        let id = self.class_member(class, name).as_reference();
        &self.records[id]
    }

    pub fn class_member_index<'a, 'b>(&'a self, class: &'a Class, name: &'b str) -> usize {
        let class_type = self.class_type(class);
        class_type
            .member_names
            .iter()
            .position(|n| n == name)
            .unwrap()
    }
}

#[derive(Debug, Clone)]
pub enum Record {
    BinaryLibrary(String),
    Class(Class),
    // ObjectArray(Vec<Member>),
    BinaryArray(MemberType, Vec<Member>),
    PrimitiveArray(PrimitiveType, Vec<Primitive>),
    // StringArray(Vec<String>),
    String(String),
}

impl Record {
    pub fn as_class(&self) -> &Class {
        if let Self::Class(class) = self {
            class
        } else {
            panic!("Record is not a Class")
        }
    }

    pub fn as_binary_array(&self) -> &[Member] {
        if let Self::BinaryArray(_, array) = self {
            array
        } else {
            panic!("Record is not an BinaryArray")
        }
    }

    pub fn as_class_mut(&mut self) -> &mut Class {
        if let Self::Class(class) = self {
            class
        } else {
            panic!("Record is not a Class")
        }
    }

    pub fn as_binary_array_mut(&mut self) -> &mut Vec<Member> {
        if let Self::BinaryArray(_, array) = self {
            array
        } else {
            panic!("Record is not an BinaryArray")
        }
    }

    pub fn as_string(&self) -> &str {
        if let Self::String(s) = self {
            s
        } else {
            panic!("Record is not a String")
        }
    }
}

#[derive(Debug, Clone)]
pub struct Class {
    pub class_type_id: usize,
    pub members: Vec<Member>,
}

#[derive(Debug, Clone)]
pub struct ClassType {
    pub name: String,
    pub library_id: i32,
    pub system_class: bool,
    pub member_names: Vec<String>,
    pub member_types: Vec<MemberType>,
}

#[derive(Debug, Clone)]
pub enum MemberType {
    Primitive(PrimitiveType),
    String,
    Object,
    SystemClass(String),
    Class(String, i32),
    ObjectArray,
    StringArray,
    PrimitiveArray(PrimitiveType),
}

#[derive(Debug, Clone)]
pub enum Member {
    Primitive(Primitive),
    Reference(i32),
    Null,
    NullMultiple(i32),
}

impl Member {
    pub fn as_reference(&self) -> &i32 {
        if let Self::Reference(id) = self {
            id
        } else {
            panic!("Member is not a Reference")
        }
    }

    pub fn as_i32(&self) -> i32 {
        if let Self::Primitive(Primitive::Int32(val)) = self {
            *val
        } else {
            panic!("Member is not a Reference")
        }
    }
}

#[derive(Debug, Clone)]
pub enum PrimitiveType {
    Boolean,
    Byte,
    Char,
    Decimal,
    Double,
    Int16,
    Int32,
    Int64,
    Int8,
    Single,
    TimeSpan,
    DateTime,
    UInt16,
    UInt32,
    UInt64,
    Null,
    String,
}

#[derive(Debug, Clone)]
pub enum Primitive {
    Boolean(bool),
    Byte(u8),
    Char(char),
    Decimal(String),
    Double(f64),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    Int8(i8),
    Single(f32),
    TimeSpan(i64),
    DateTime(i64),
    UInt16(u16),
    UInt32(u32),
    UInt64(u64),
    Null,
    String(String),
}

impl Primitive {
    pub fn primitive_type(&self) -> PrimitiveType {
        match self {
            Primitive::Boolean(..) => PrimitiveType::Boolean,
            Primitive::Byte(..) => PrimitiveType::Byte,
            Primitive::Char(..) => PrimitiveType::Char,
            Primitive::Decimal(..) => PrimitiveType::Decimal,
            Primitive::Double(..) => PrimitiveType::Double,
            Primitive::Int16(..) => PrimitiveType::Int16,
            Primitive::Int32(..) => PrimitiveType::Int32,
            Primitive::Int64(..) => PrimitiveType::Int64,
            Primitive::Int8(..) => PrimitiveType::Int8,
            Primitive::Single(..) => PrimitiveType::Single,
            Primitive::TimeSpan(..) => PrimitiveType::TimeSpan,
            Primitive::DateTime(..) => PrimitiveType::DateTime,
            Primitive::UInt16(..) => PrimitiveType::UInt16,
            Primitive::UInt32(..) => PrimitiveType::UInt32,
            Primitive::UInt64(..) => PrimitiveType::UInt64,
            Primitive::Null => PrimitiveType::Null,
            Primitive::String(..) => PrimitiveType::String,
        }
    }
}
