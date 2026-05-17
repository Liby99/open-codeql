//! Parser for JVM `.class` files (ClassFile format).
//!
//! Reads the binary ClassFile structure as defined in the JVM specification and
//! produces a high-level [`ClassFile`] with resolved string names.

use std::fmt;

// ---------------------------------------------------------------------------
// Access flag constants
// ---------------------------------------------------------------------------

pub const ACC_PUBLIC: u16 = 0x0001;
pub const ACC_PRIVATE: u16 = 0x0002;
pub const ACC_PROTECTED: u16 = 0x0004;
pub const ACC_STATIC: u16 = 0x0008;
pub const ACC_FINAL: u16 = 0x0010;
pub const ACC_SYNCHRONIZED: u16 = 0x0020;
pub const ACC_VOLATILE: u16 = 0x0040;
pub const ACC_TRANSIENT: u16 = 0x0080;
pub const ACC_NATIVE: u16 = 0x0100;
pub const ACC_INTERFACE: u16 = 0x0200;
pub const ACC_ABSTRACT: u16 = 0x0400;
pub const ACC_STRICT: u16 = 0x0800;
pub const ACC_SYNTHETIC: u16 = 0x1000;
pub const ACC_ANNOTATION: u16 = 0x2000;
pub const ACC_ENUM: u16 = 0x4000;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
}

impl ParseError {
    fn new(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ClassFile parse error: {}", self.message)
    }
}

impl std::error::Error for ParseError {}

// ---------------------------------------------------------------------------
// Public output structures
// ---------------------------------------------------------------------------

/// A parsed JVM `.class` file with resolved names.
#[derive(Debug, Clone)]
pub struct ClassFile {
    pub major_version: u16,
    pub minor_version: u16,
    pub access_flags: u16,
    /// Fully-qualified internal name, e.g. `"java/lang/String"`.
    pub this_class: String,
    /// `None` for `java/lang/Object` (super_class index == 0).
    pub super_class: Option<String>,
    pub interfaces: Vec<String>,
    pub fields: Vec<FieldInfo>,
    pub methods: Vec<MethodInfo>,
    /// String constants from the constant pool (CONSTANT_String entries).
    pub string_constants: Vec<String>,
}

/// A field declared in the class file.
#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub access_flags: u16,
    pub name: String,
    /// JVM field descriptor, e.g. `"I"`, `"Ljava/lang/String;"`.
    pub descriptor: String,
}

/// A method declared in the class file.
#[derive(Debug, Clone)]
pub struct MethodInfo {
    pub access_flags: u16,
    pub name: String,
    /// JVM method descriptor, e.g. `"(Ljava/lang/String;)V"`.
    pub descriptor: String,
}

// ---------------------------------------------------------------------------
// Constant pool (internal)
// ---------------------------------------------------------------------------

/// One entry in the constant pool.  We only keep the data we need to resolve
/// names; numeric literals are stored but not exposed in the public API.
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum CpEntry {
    /// Placeholder for index 0 and the second slot of Long/Double.
    Unusable,
    Utf8(String),
    Integer(i32),
    Float(f32),
    Long(i64),
    Double(f64),
    /// Points to a Utf8 entry with the internal class name.
    Class(u16),
    /// Points to a Utf8 entry.
    StringRef(u16),
    /// (class_index, name_and_type_index)
    Fieldref(u16, u16),
    /// (class_index, name_and_type_index)
    Methodref(u16, u16),
    /// (class_index, name_and_type_index)
    InterfaceMethodref(u16, u16),
    /// (name_index, descriptor_index)
    NameAndType(u16, u16),
    /// (reference_kind, reference_index)
    MethodHandle(u8, u16),
    /// Points to a Utf8 entry with the method descriptor.
    MethodType(u16),
    /// (bootstrap_method_attr_index, name_and_type_index)
    Dynamic(u16, u16),
    /// (bootstrap_method_attr_index, name_and_type_index)
    InvokeDynamic(u16, u16),
    /// Points to a Utf8 entry with the module name.
    Module(u16),
    /// Points to a Utf8 entry with the package name.
    Package(u16),
}

// ---------------------------------------------------------------------------
// Low-level reader
// ---------------------------------------------------------------------------

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn ensure(&self, n: usize) -> Result<(), ParseError> {
        if self.remaining() < n {
            Err(ParseError::new(format!(
                "unexpected end of data at offset {} (need {} bytes, have {})",
                self.pos,
                n,
                self.remaining()
            )))
        } else {
            Ok(())
        }
    }

    fn read_u8(&mut self) -> Result<u8, ParseError> {
        self.ensure(1)?;
        let v = self.data[self.pos];
        self.pos += 1;
        Ok(v)
    }

    fn read_u16(&mut self) -> Result<u16, ParseError> {
        self.ensure(2)?;
        let v = u16::from_be_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }

    fn read_u32(&mut self) -> Result<u32, ParseError> {
        self.ensure(4)?;
        let v = u32::from_be_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], ParseError> {
        self.ensure(n)?;
        let slice = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    fn skip(&mut self, n: usize) -> Result<(), ParseError> {
        self.ensure(n)?;
        self.pos += n;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Constant pool helpers
// ---------------------------------------------------------------------------

struct ConstantPool {
    entries: Vec<CpEntry>,
}

impl ConstantPool {
    fn get(&self, index: u16) -> Result<&CpEntry, ParseError> {
        let i = index as usize;
        if i == 0 || i >= self.entries.len() {
            return Err(ParseError::new(format!(
                "constant pool index {} out of range (pool size {})",
                index,
                self.entries.len()
            )));
        }
        Ok(&self.entries[i])
    }

    /// Resolve a Utf8 entry.
    fn get_utf8(&self, index: u16) -> Result<&str, ParseError> {
        match self.get(index)? {
            CpEntry::Utf8(s) => Ok(s.as_str()),
            other => Err(ParseError::new(format!(
                "expected Utf8 at constant pool index {}, found {:?}",
                index, other
            ))),
        }
    }

    /// Resolve a CONSTANT_Class entry to its internal name string.
    fn get_class_name(&self, index: u16) -> Result<&str, ParseError> {
        match self.get(index)? {
            CpEntry::Class(name_index) => self.get_utf8(*name_index),
            other => Err(ParseError::new(format!(
                "expected Class at constant pool index {}, found {:?}",
                index, other
            ))),
        }
    }

    /// Collect all CONSTANT_String entries (resolved to their UTF-8 values).
    fn collect_string_constants(&self) -> Vec<String> {
        let mut result = Vec::new();
        for entry in &self.entries {
            if let CpEntry::StringRef(utf8_idx) = entry {
                if let Ok(s) = self.get_utf8(*utf8_idx) {
                    result.push(s.to_string());
                }
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// Main parser
// ---------------------------------------------------------------------------

/// Parse a JVM `.class` file from raw bytes.
pub fn parse_class(data: &[u8]) -> Result<ClassFile, ParseError> {
    let mut r = Reader::new(data);

    // Magic number
    let magic = r.read_u32()?;
    if magic != 0xCAFEBABE {
        return Err(ParseError::new(format!(
            "invalid magic number: 0x{:08X} (expected 0xCAFEBABE)",
            magic
        )));
    }

    // Version
    let minor_version = r.read_u16()?;
    let major_version = r.read_u16()?;

    // Constant pool
    let cp_count = r.read_u16()?;
    let cp = read_constant_pool(&mut r, cp_count)?;

    // Access flags, this class, super class
    let access_flags = r.read_u16()?;

    let this_class_idx = r.read_u16()?;
    let this_class = cp.get_class_name(this_class_idx)?.to_owned();

    let super_class_idx = r.read_u16()?;
    let super_class = if super_class_idx == 0 {
        None
    } else {
        Some(cp.get_class_name(super_class_idx)?.to_owned())
    };

    // Interfaces
    let interfaces_count = r.read_u16()?;
    let mut interfaces = Vec::with_capacity(interfaces_count as usize);
    for _ in 0..interfaces_count {
        let idx = r.read_u16()?;
        interfaces.push(cp.get_class_name(idx)?.to_owned());
    }

    // Fields
    let fields_count = r.read_u16()?;
    let mut fields = Vec::with_capacity(fields_count as usize);
    for _ in 0..fields_count {
        fields.push(read_field_or_method(&mut r, &cp, "field")?);
    }

    // Methods
    let methods_count = r.read_u16()?;
    let mut methods = Vec::with_capacity(methods_count as usize);
    for _ in 0..methods_count {
        methods.push(read_field_or_method(&mut r, &cp, "method")?);
    }

    // Collect string constants from constant pool
    let string_constants = cp.collect_string_constants();

    // Class-level attributes (skip)
    skip_attributes(&mut r)?;

    Ok(ClassFile {
        major_version,
        minor_version,
        access_flags,
        this_class,
        super_class,
        interfaces,
        string_constants,
        fields: fields
            .into_iter()
            .map(|(af, n, d)| FieldInfo {
                access_flags: af,
                name: n,
                descriptor: d,
            })
            .collect(),
        methods: methods
            .into_iter()
            .map(|(af, n, d)| MethodInfo {
                access_flags: af,
                name: n,
                descriptor: d,
            })
            .collect(),
    })
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn read_constant_pool(r: &mut Reader, cp_count: u16) -> Result<ConstantPool, ParseError> {
    // Index 0 is unused; the pool runs from 1..cp_count-1.
    let mut entries: Vec<CpEntry> = Vec::with_capacity(cp_count as usize);
    entries.push(CpEntry::Unusable); // slot 0

    let mut i: u16 = 1;
    while i < cp_count {
        let tag = r.read_u8()?;
        let entry = match tag {
            1 => {
                // CONSTANT_Utf8
                let len = r.read_u16()? as usize;
                let bytes = r.read_bytes(len)?;
                // Modified UTF-8: for the vast majority of real class files plain
                // from_utf8 works.  Fall back to lossy for the rare edge case.
                let s = match std::str::from_utf8(bytes) {
                    Ok(s) => s.to_owned(),
                    Err(_) => decode_modified_utf8(bytes),
                };
                CpEntry::Utf8(s)
            }
            3 => {
                // CONSTANT_Integer
                let bits = r.read_u32()?;
                CpEntry::Integer(bits as i32)
            }
            4 => {
                // CONSTANT_Float
                let bits = r.read_u32()?;
                CpEntry::Float(f32::from_bits(bits))
            }
            5 => {
                // CONSTANT_Long — occupies TWO slots
                let high = r.read_u32()? as u64;
                let low = r.read_u32()? as u64;
                let val = ((high << 32) | low) as i64;
                let entry = CpEntry::Long(val);
                entries.push(entry);
                entries.push(CpEntry::Unusable);
                i += 2;
                continue;
            }
            6 => {
                // CONSTANT_Double — occupies TWO slots
                let high = r.read_u32()? as u64;
                let low = r.read_u32()? as u64;
                let bits = (high << 32) | low;
                let entry = CpEntry::Double(f64::from_bits(bits));
                entries.push(entry);
                entries.push(CpEntry::Unusable);
                i += 2;
                continue;
            }
            7 => {
                // CONSTANT_Class
                let name_index = r.read_u16()?;
                CpEntry::Class(name_index)
            }
            8 => {
                // CONSTANT_String
                let string_index = r.read_u16()?;
                CpEntry::StringRef(string_index)
            }
            9 => {
                // CONSTANT_Fieldref
                let class_index = r.read_u16()?;
                let nat_index = r.read_u16()?;
                CpEntry::Fieldref(class_index, nat_index)
            }
            10 => {
                // CONSTANT_Methodref
                let class_index = r.read_u16()?;
                let nat_index = r.read_u16()?;
                CpEntry::Methodref(class_index, nat_index)
            }
            11 => {
                // CONSTANT_InterfaceMethodref
                let class_index = r.read_u16()?;
                let nat_index = r.read_u16()?;
                CpEntry::InterfaceMethodref(class_index, nat_index)
            }
            12 => {
                // CONSTANT_NameAndType
                let name_index = r.read_u16()?;
                let descriptor_index = r.read_u16()?;
                CpEntry::NameAndType(name_index, descriptor_index)
            }
            15 => {
                // CONSTANT_MethodHandle
                let reference_kind = r.read_u8()?;
                let reference_index = r.read_u16()?;
                CpEntry::MethodHandle(reference_kind, reference_index)
            }
            16 => {
                // CONSTANT_MethodType
                let descriptor_index = r.read_u16()?;
                CpEntry::MethodType(descriptor_index)
            }
            17 => {
                // CONSTANT_Dynamic (tag 17, Java 11+)
                let bootstrap_method_attr_index = r.read_u16()?;
                let nat_index = r.read_u16()?;
                CpEntry::Dynamic(bootstrap_method_attr_index, nat_index)
            }
            18 => {
                // CONSTANT_InvokeDynamic
                let bootstrap_method_attr_index = r.read_u16()?;
                let nat_index = r.read_u16()?;
                CpEntry::InvokeDynamic(bootstrap_method_attr_index, nat_index)
            }
            19 => {
                // CONSTANT_Module (Java 9+)
                let name_index = r.read_u16()?;
                CpEntry::Module(name_index)
            }
            20 => {
                // CONSTANT_Package (Java 9+)
                let name_index = r.read_u16()?;
                CpEntry::Package(name_index)
            }
            _ => {
                return Err(ParseError::new(format!(
                    "unknown constant pool tag {} at pool index {}",
                    tag, i
                )));
            }
        };

        entries.push(entry);
        i += 1;
    }

    Ok(ConstantPool { entries })
}

/// Read a field_info or method_info structure.  Returns (access_flags, name, descriptor).
fn read_field_or_method(
    r: &mut Reader,
    cp: &ConstantPool,
    _kind: &str,
) -> Result<(u16, String, String), ParseError> {
    let access_flags = r.read_u16()?;
    let name_index = r.read_u16()?;
    let descriptor_index = r.read_u16()?;
    let name = cp.get_utf8(name_index)?.to_owned();
    let descriptor = cp.get_utf8(descriptor_index)?.to_owned();

    // Skip attributes
    skip_attributes(r)?;

    Ok((access_flags, name, descriptor))
}

/// Skip an `attributes_count + attribute_info[]` block.
fn skip_attributes(r: &mut Reader) -> Result<(), ParseError> {
    let count = r.read_u16()?;
    for _ in 0..count {
        // attribute_name_index (u16) — we don't need it
        r.skip(2)?;
        // attribute_length (u32)
        let len = r.read_u32()? as usize;
        r.skip(len)?;
    }
    Ok(())
}

/// Decode JVM Modified UTF-8 into a Rust String.
///
/// Modified UTF-8 differs from standard UTF-8:
/// - The null character U+0000 is encoded as `0xC0 0x80` (two bytes).
/// - Supplementary characters (U+10000+) are encoded as a surrogate pair,
///   each surrogate encoded in 3 bytes of CESU-8.
///
/// For the vast majority of class files standard UTF-8 works fine, but this
/// function handles the edge cases.
fn decode_modified_utf8(bytes: &[u8]) -> String {
    let mut result = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == 0 {
            // Should not appear in modified UTF-8, but handle gracefully.
            result.push('\0');
            i += 1;
        } else if b & 0x80 == 0 {
            // Single-byte (ASCII)
            result.push(b as char);
            i += 1;
        } else if b & 0xE0 == 0xC0 {
            // Two-byte
            if i + 1 >= bytes.len() {
                result.push(char::REPLACEMENT_CHARACTER);
                break;
            }
            let c = (((b & 0x1F) as u32) << 6) | ((bytes[i + 1] & 0x3F) as u32);
            result.push(char::from_u32(c).unwrap_or(char::REPLACEMENT_CHARACTER));
            i += 2;
        } else if b & 0xF0 == 0xE0 {
            // Three-byte — could be a surrogate in CESU-8
            if i + 2 >= bytes.len() {
                result.push(char::REPLACEMENT_CHARACTER);
                break;
            }
            let c = (((b & 0x0F) as u32) << 12)
                | (((bytes[i + 1] & 0x3F) as u32) << 6)
                | ((bytes[i + 2] & 0x3F) as u32);

            if (0xD800..=0xDBFF).contains(&c) {
                // High surrogate — look for low surrogate
                if i + 5 < bytes.len()
                    && bytes[i + 3] & 0xF0 == 0xE0
                {
                    let c2 = (((bytes[i + 3] & 0x0F) as u32) << 12)
                        | (((bytes[i + 4] & 0x3F) as u32) << 6)
                        | ((bytes[i + 5] & 0x3F) as u32);
                    if (0xDC00..=0xDFFF).contains(&c2) {
                        let codepoint =
                            0x10000 + ((c - 0xD800) << 10) + (c2 - 0xDC00);
                        result.push(
                            char::from_u32(codepoint)
                                .unwrap_or(char::REPLACEMENT_CHARACTER),
                        );
                        i += 6;
                        continue;
                    }
                }
                // Lone surrogate — replacement
                result.push(char::REPLACEMENT_CHARACTER);
                i += 3;
            } else {
                result.push(char::from_u32(c).unwrap_or(char::REPLACEMENT_CHARACTER));
                i += 3;
            }
        } else {
            // Invalid leading byte
            result.push(char::REPLACEMENT_CHARACTER);
            i += 1;
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Descriptor utilities
// ---------------------------------------------------------------------------

/// Parse a JVM method descriptor into parameter descriptors and a return type
/// descriptor.
///
/// # Example
/// ```
/// # use ocql_extractor_java::bytecode::parse_method_descriptor;
/// let (params, ret) = parse_method_descriptor("(Ljava/lang/String;IZ)V");
/// assert_eq!(params, vec!["Ljava/lang/String;", "I", "Z"]);
/// assert_eq!(ret, "V");
/// ```
pub fn parse_method_descriptor(desc: &str) -> (Vec<String>, String) {
    let bytes = desc.as_bytes();
    if bytes.is_empty() || bytes[0] != b'(' {
        return (vec![], desc.to_owned());
    }

    let mut params = Vec::new();
    let mut i = 1; // skip '('

    while i < bytes.len() && bytes[i] != b')' {
        let (ty, next) = read_one_type(bytes, i);
        params.push(ty);
        i = next;
    }

    // Skip ')'
    if i < bytes.len() && bytes[i] == b')' {
        i += 1;
    }

    let ret = if i < bytes.len() {
        let (ty, _) = read_one_type(bytes, i);
        ty
    } else {
        String::new()
    };

    (params, ret)
}

/// Read one type descriptor starting at `pos` and return (descriptor, next_pos).
fn read_one_type(bytes: &[u8], pos: usize) -> (String, usize) {
    if pos >= bytes.len() {
        return (String::new(), pos);
    }
    match bytes[pos] {
        b'B' | b'C' | b'D' | b'F' | b'I' | b'J' | b'S' | b'Z' | b'V' => {
            (String::from(bytes[pos] as char), pos + 1)
        }
        b'L' => {
            // Object type: Lclassname;
            if let Some(semi) = bytes[pos..].iter().position(|&b| b == b';') {
                let end = pos + semi + 1; // include the ';'
                let s = std::str::from_utf8(&bytes[pos..end])
                    .unwrap_or("")
                    .to_owned();
                (s, end)
            } else {
                // Malformed — consume the rest
                let s = std::str::from_utf8(&bytes[pos..])
                    .unwrap_or("")
                    .to_owned();
                (s, bytes.len())
            }
        }
        b'[' => {
            // Array type: [<type>
            let (inner, next) = read_one_type(bytes, pos + 1);
            (format!("[{}", inner), next)
        }
        _ => {
            // Unknown — consume one byte
            (String::from(bytes[pos] as char), pos + 1)
        }
    }
}

/// Convert a JVM type descriptor to a human-readable Java type name.
///
/// # Examples
/// ```
/// # use ocql_extractor_java::bytecode::descriptor_to_type_name;
/// assert_eq!(descriptor_to_type_name("I"), "int");
/// assert_eq!(descriptor_to_type_name("Ljava/lang/String;"), "String");
/// assert_eq!(descriptor_to_type_name("[I"), "int[]");
/// assert_eq!(descriptor_to_type_name("[[Ljava/lang/Object;"), "Object[][]");
/// assert_eq!(descriptor_to_type_name("V"), "void");
/// ```
pub fn descriptor_to_type_name(desc: &str) -> String {
    let bytes = desc.as_bytes();
    if bytes.is_empty() {
        return String::new();
    }

    // Count leading '[' for array dimensions
    let mut dims = 0usize;
    while dims < bytes.len() && bytes[dims] == b'[' {
        dims += 1;
    }

    let base = &desc[dims..];
    let base_name = match base.as_bytes().first() {
        Some(b'B') => "byte".to_owned(),
        Some(b'C') => "char".to_owned(),
        Some(b'D') => "double".to_owned(),
        Some(b'F') => "float".to_owned(),
        Some(b'I') => "int".to_owned(),
        Some(b'J') => "long".to_owned(),
        Some(b'S') => "short".to_owned(),
        Some(b'Z') => "boolean".to_owned(),
        Some(b'V') => "void".to_owned(),
        Some(b'L') => {
            // Lcom/example/Foo; -> simple name "Foo"
            let inner = base
                .strip_prefix('L')
                .and_then(|s| s.strip_suffix(';'))
                .unwrap_or(base);
            // Take the simple name (after last '/')
            inner
                .rsplit('/')
                .next()
                .unwrap_or(inner)
                // Also handle inner classes separated by '$' — take last segment
                .rsplit('$')
                .next()
                .unwrap_or(inner)
                .to_owned()
        }
        _ => base.to_owned(),
    };

    let mut result = base_name;
    for _ in 0..dims {
        result.push_str("[]");
    }
    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: build a minimal valid class file for testing.
    fn build_minimal_class() -> Vec<u8> {
        let mut buf = Vec::new();

        // Magic
        buf.extend_from_slice(&0xCAFEBABEu32.to_be_bytes());

        // Version: Java 8 (52.0)
        buf.extend_from_slice(&0u16.to_be_bytes()); // minor
        buf.extend_from_slice(&52u16.to_be_bytes()); // major

        // Constant pool count: 1 (index 0) + entries
        // We need:
        //   #1: Utf8 "MyClass"
        //   #2: Class -> #1
        //   #3: Utf8 "java/lang/Object"
        //   #4: Class -> #3
        //   #5: Utf8 "java/io/Serializable"
        //   #6: Class -> #5
        //   #7: Utf8 "value"
        //   #8: Utf8 "I"
        //   #9: Utf8 "getValue"
        //   #10: Utf8 "()I"
        // cp_count = 11
        buf.extend_from_slice(&11u16.to_be_bytes());

        // #1 Utf8 "MyClass"
        buf.push(1); // tag
        let s = b"MyClass";
        buf.extend_from_slice(&(s.len() as u16).to_be_bytes());
        buf.extend_from_slice(s);

        // #2 Class -> #1
        buf.push(7);
        buf.extend_from_slice(&1u16.to_be_bytes());

        // #3 Utf8 "java/lang/Object"
        buf.push(1);
        let s = b"java/lang/Object";
        buf.extend_from_slice(&(s.len() as u16).to_be_bytes());
        buf.extend_from_slice(s);

        // #4 Class -> #3
        buf.push(7);
        buf.extend_from_slice(&3u16.to_be_bytes());

        // #5 Utf8 "java/io/Serializable"
        buf.push(1);
        let s = b"java/io/Serializable";
        buf.extend_from_slice(&(s.len() as u16).to_be_bytes());
        buf.extend_from_slice(s);

        // #6 Class -> #5
        buf.push(7);
        buf.extend_from_slice(&5u16.to_be_bytes());

        // #7 Utf8 "value"
        buf.push(1);
        let s = b"value";
        buf.extend_from_slice(&(s.len() as u16).to_be_bytes());
        buf.extend_from_slice(s);

        // #8 Utf8 "I"
        buf.push(1);
        let s = b"I";
        buf.extend_from_slice(&(s.len() as u16).to_be_bytes());
        buf.extend_from_slice(s);

        // #9 Utf8 "getValue"
        buf.push(1);
        let s = b"getValue";
        buf.extend_from_slice(&(s.len() as u16).to_be_bytes());
        buf.extend_from_slice(s);

        // #10 Utf8 "()I"
        buf.push(1);
        let s = b"()I";
        buf.extend_from_slice(&(s.len() as u16).to_be_bytes());
        buf.extend_from_slice(s);

        // Access flags: public
        buf.extend_from_slice(&ACC_PUBLIC.to_be_bytes());

        // this_class: #2
        buf.extend_from_slice(&2u16.to_be_bytes());

        // super_class: #4
        buf.extend_from_slice(&4u16.to_be_bytes());

        // interfaces_count: 1
        buf.extend_from_slice(&1u16.to_be_bytes());
        // interfaces[0]: #6
        buf.extend_from_slice(&6u16.to_be_bytes());

        // fields_count: 1
        buf.extend_from_slice(&1u16.to_be_bytes());
        // field: private int value
        buf.extend_from_slice(&ACC_PRIVATE.to_be_bytes()); // access_flags
        buf.extend_from_slice(&7u16.to_be_bytes()); // name_index -> "value"
        buf.extend_from_slice(&8u16.to_be_bytes()); // descriptor_index -> "I"
        buf.extend_from_slice(&0u16.to_be_bytes()); // attributes_count: 0

        // methods_count: 1
        buf.extend_from_slice(&1u16.to_be_bytes());
        // method: public int getValue()
        buf.extend_from_slice(&ACC_PUBLIC.to_be_bytes()); // access_flags
        buf.extend_from_slice(&9u16.to_be_bytes()); // name_index -> "getValue"
        buf.extend_from_slice(&10u16.to_be_bytes()); // descriptor_index -> "()I"
        buf.extend_from_slice(&0u16.to_be_bytes()); // attributes_count: 0

        // class attributes_count: 0
        buf.extend_from_slice(&0u16.to_be_bytes());

        buf
    }

    #[test]
    fn parse_minimal_class() {
        let data = build_minimal_class();
        let cf = parse_class(&data).expect("parse_class failed");

        assert_eq!(cf.major_version, 52);
        assert_eq!(cf.minor_version, 0);
        assert_eq!(cf.access_flags, ACC_PUBLIC);
        assert_eq!(cf.this_class, "MyClass");
        assert_eq!(cf.super_class.as_deref(), Some("java/lang/Object"));
        assert_eq!(cf.interfaces, vec!["java/io/Serializable"]);

        assert_eq!(cf.fields.len(), 1);
        assert_eq!(cf.fields[0].name, "value");
        assert_eq!(cf.fields[0].descriptor, "I");
        assert_eq!(cf.fields[0].access_flags, ACC_PRIVATE);

        assert_eq!(cf.methods.len(), 1);
        assert_eq!(cf.methods[0].name, "getValue");
        assert_eq!(cf.methods[0].descriptor, "()I");
        assert_eq!(cf.methods[0].access_flags, ACC_PUBLIC);
    }

    #[test]
    fn bad_magic() {
        let data = [0x00, 0x00, 0x00, 0x00];
        let err = parse_class(&data).unwrap_err();
        assert!(err.message.contains("invalid magic"));
    }

    #[test]
    fn truncated_data() {
        let data = [0xCA, 0xFE, 0xBA, 0xBE, 0x00];
        let err = parse_class(&data).unwrap_err();
        assert!(err.message.contains("unexpected end"));
    }

    #[test]
    fn test_parse_method_descriptor_basic() {
        let (params, ret) = parse_method_descriptor("(Ljava/lang/String;IZ)V");
        assert_eq!(params, vec!["Ljava/lang/String;", "I", "Z"]);
        assert_eq!(ret, "V");
    }

    #[test]
    fn test_parse_method_descriptor_no_params() {
        let (params, ret) = parse_method_descriptor("()V");
        assert!(params.is_empty());
        assert_eq!(ret, "V");
    }

    #[test]
    fn test_parse_method_descriptor_array_params() {
        let (params, ret) = parse_method_descriptor("([Ljava/lang/String;)I");
        assert_eq!(params, vec!["[Ljava/lang/String;"]);
        assert_eq!(ret, "I");
    }

    #[test]
    fn test_parse_method_descriptor_multi_dim_array() {
        let (params, ret) = parse_method_descriptor("([[I[D)Ljava/lang/Object;");
        assert_eq!(params, vec!["[[I", "[D"]);
        assert_eq!(ret, "Ljava/lang/Object;");
    }

    #[test]
    fn test_descriptor_to_type_name_primitives() {
        assert_eq!(descriptor_to_type_name("I"), "int");
        assert_eq!(descriptor_to_type_name("J"), "long");
        assert_eq!(descriptor_to_type_name("D"), "double");
        assert_eq!(descriptor_to_type_name("F"), "float");
        assert_eq!(descriptor_to_type_name("B"), "byte");
        assert_eq!(descriptor_to_type_name("C"), "char");
        assert_eq!(descriptor_to_type_name("S"), "short");
        assert_eq!(descriptor_to_type_name("Z"), "boolean");
        assert_eq!(descriptor_to_type_name("V"), "void");
    }

    #[test]
    fn test_descriptor_to_type_name_object() {
        assert_eq!(descriptor_to_type_name("Ljava/lang/String;"), "String");
        assert_eq!(descriptor_to_type_name("Ljava/util/Map;"), "Map");
    }

    #[test]
    fn test_descriptor_to_type_name_array() {
        assert_eq!(descriptor_to_type_name("[I"), "int[]");
        assert_eq!(descriptor_to_type_name("[[I"), "int[][]");
        assert_eq!(
            descriptor_to_type_name("[Ljava/lang/Object;"),
            "Object[]"
        );
        assert_eq!(
            descriptor_to_type_name("[[Ljava/lang/Object;"),
            "Object[][]"
        );
    }

    #[test]
    fn test_descriptor_to_type_name_inner_class() {
        assert_eq!(
            descriptor_to_type_name("Ljava/util/Map$Entry;"),
            "Entry"
        );
    }

    #[test]
    fn test_long_double_two_slots() {
        // Build a class file with a Long constant in the constant pool to
        // verify that the two-slot handling works correctly.
        let mut buf = Vec::new();

        // Magic
        buf.extend_from_slice(&0xCAFEBABEu32.to_be_bytes());
        // Version
        buf.extend_from_slice(&0u16.to_be_bytes());
        buf.extend_from_slice(&52u16.to_be_bytes());

        // Constant pool:
        //   #1: Utf8 "Test"
        //   #2: Class -> #1
        //   #3: Long 42  (takes slots #3 and #4)
        //   #5: Utf8 "java/lang/Object"
        //   #6: Class -> #5
        // cp_count = 7
        buf.extend_from_slice(&7u16.to_be_bytes());

        // #1 Utf8 "Test"
        buf.push(1);
        let s = b"Test";
        buf.extend_from_slice(&(s.len() as u16).to_be_bytes());
        buf.extend_from_slice(s);

        // #2 Class -> #1
        buf.push(7);
        buf.extend_from_slice(&1u16.to_be_bytes());

        // #3 Long 42 (takes two slots: #3 and #4)
        buf.push(5); // CONSTANT_Long tag
        buf.extend_from_slice(&0u32.to_be_bytes()); // high
        buf.extend_from_slice(&42u32.to_be_bytes()); // low

        // #5 Utf8 "java/lang/Object"
        buf.push(1);
        let s = b"java/lang/Object";
        buf.extend_from_slice(&(s.len() as u16).to_be_bytes());
        buf.extend_from_slice(s);

        // #6 Class -> #5
        buf.push(7);
        buf.extend_from_slice(&5u16.to_be_bytes());

        // Access flags
        buf.extend_from_slice(&ACC_PUBLIC.to_be_bytes());
        // this_class: #2
        buf.extend_from_slice(&2u16.to_be_bytes());
        // super_class: #6
        buf.extend_from_slice(&6u16.to_be_bytes());
        // interfaces: 0
        buf.extend_from_slice(&0u16.to_be_bytes());
        // fields: 0
        buf.extend_from_slice(&0u16.to_be_bytes());
        // methods: 0
        buf.extend_from_slice(&0u16.to_be_bytes());
        // attributes: 0
        buf.extend_from_slice(&0u16.to_be_bytes());

        let cf = parse_class(&buf).expect("parse_class failed for Long test");
        assert_eq!(cf.this_class, "Test");
        assert_eq!(cf.super_class.as_deref(), Some("java/lang/Object"));
    }

    #[test]
    fn test_no_super_class() {
        // Build a class file where super_class index is 0 (like java.lang.Object itself).
        let mut buf = Vec::new();

        buf.extend_from_slice(&0xCAFEBABEu32.to_be_bytes());
        buf.extend_from_slice(&0u16.to_be_bytes());
        buf.extend_from_slice(&52u16.to_be_bytes());

        // cp_count = 3: #1 Utf8, #2 Class
        buf.extend_from_slice(&3u16.to_be_bytes());

        // #1 Utf8 "java/lang/Object"
        buf.push(1);
        let s = b"java/lang/Object";
        buf.extend_from_slice(&(s.len() as u16).to_be_bytes());
        buf.extend_from_slice(s);

        // #2 Class -> #1
        buf.push(7);
        buf.extend_from_slice(&1u16.to_be_bytes());

        // access_flags
        buf.extend_from_slice(&ACC_PUBLIC.to_be_bytes());
        // this_class: #2
        buf.extend_from_slice(&2u16.to_be_bytes());
        // super_class: 0 (none)
        buf.extend_from_slice(&0u16.to_be_bytes());
        // interfaces: 0
        buf.extend_from_slice(&0u16.to_be_bytes());
        // fields: 0
        buf.extend_from_slice(&0u16.to_be_bytes());
        // methods: 0
        buf.extend_from_slice(&0u16.to_be_bytes());
        // attributes: 0
        buf.extend_from_slice(&0u16.to_be_bytes());

        let cf = parse_class(&buf).expect("parse_class failed for Object test");
        assert_eq!(cf.this_class, "java/lang/Object");
        assert!(cf.super_class.is_none());
    }

    #[test]
    fn test_field_with_attributes_skipped() {
        // Ensure attribute skipping works: a field with a synthetic ConstantValue attribute.
        let mut buf = Vec::new();

        buf.extend_from_slice(&0xCAFEBABEu32.to_be_bytes());
        buf.extend_from_slice(&0u16.to_be_bytes());
        buf.extend_from_slice(&52u16.to_be_bytes());

        // Constant pool:
        //   #1: Utf8 "Foo"
        //   #2: Class -> #1
        //   #3: Utf8 "java/lang/Object"
        //   #4: Class -> #3
        //   #5: Utf8 "x"
        //   #6: Utf8 "I"
        //   #7: Utf8 "ConstantValue"
        //   #8: Integer 99
        // cp_count = 9
        buf.extend_from_slice(&9u16.to_be_bytes());

        // #1
        buf.push(1);
        let s = b"Foo";
        buf.extend_from_slice(&(s.len() as u16).to_be_bytes());
        buf.extend_from_slice(s);
        // #2
        buf.push(7);
        buf.extend_from_slice(&1u16.to_be_bytes());
        // #3
        buf.push(1);
        let s = b"java/lang/Object";
        buf.extend_from_slice(&(s.len() as u16).to_be_bytes());
        buf.extend_from_slice(s);
        // #4
        buf.push(7);
        buf.extend_from_slice(&3u16.to_be_bytes());
        // #5
        buf.push(1);
        let s = b"x";
        buf.extend_from_slice(&(s.len() as u16).to_be_bytes());
        buf.extend_from_slice(s);
        // #6
        buf.push(1);
        let s = b"I";
        buf.extend_from_slice(&(s.len() as u16).to_be_bytes());
        buf.extend_from_slice(s);
        // #7
        buf.push(1);
        let s = b"ConstantValue";
        buf.extend_from_slice(&(s.len() as u16).to_be_bytes());
        buf.extend_from_slice(s);
        // #8 Integer
        buf.push(3);
        buf.extend_from_slice(&99u32.to_be_bytes());

        // Access flags
        buf.extend_from_slice(&ACC_PUBLIC.to_be_bytes());
        // this_class
        buf.extend_from_slice(&2u16.to_be_bytes());
        // super_class
        buf.extend_from_slice(&4u16.to_be_bytes());
        // interfaces: 0
        buf.extend_from_slice(&0u16.to_be_bytes());

        // fields: 1
        buf.extend_from_slice(&1u16.to_be_bytes());
        buf.extend_from_slice(&(ACC_PUBLIC | ACC_STATIC | ACC_FINAL).to_be_bytes());
        buf.extend_from_slice(&5u16.to_be_bytes()); // name -> "x"
        buf.extend_from_slice(&6u16.to_be_bytes()); // descriptor -> "I"
        // 1 attribute: ConstantValue (attribute_name_index=#7, length=2, constantvalue_index=#8)
        buf.extend_from_slice(&1u16.to_be_bytes()); // attributes_count
        buf.extend_from_slice(&7u16.to_be_bytes()); // attr name index
        buf.extend_from_slice(&2u32.to_be_bytes()); // attr length
        buf.extend_from_slice(&8u16.to_be_bytes()); // constantvalue_index

        // methods: 0
        buf.extend_from_slice(&0u16.to_be_bytes());
        // class attributes: 0
        buf.extend_from_slice(&0u16.to_be_bytes());

        let cf = parse_class(&buf).expect("parse_class with attr failed");
        assert_eq!(cf.fields.len(), 1);
        assert_eq!(cf.fields[0].name, "x");
        assert_eq!(
            cf.fields[0].access_flags,
            ACC_PUBLIC | ACC_STATIC | ACC_FINAL
        );
    }
}
