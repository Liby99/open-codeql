use std::collections::HashMap;

/// Interned string handle (u32 index into string table).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InternedString(pub u32);

/// Global string table for the database. All strings are interned
/// so that equality checks are O(1) and storage is deduplicated.
pub struct StringInterner {
    strings: Vec<String>,
    lookup: HashMap<String, InternedString>,
}

impl StringInterner {
    pub fn new() -> Self {
        Self {
            strings: Vec::new(),
            lookup: HashMap::new(),
        }
    }

    /// Intern a string, returning its handle.
    /// If the string was already interned, returns the existing handle.
    pub fn intern(&mut self, s: &str) -> InternedString {
        if let Some(&id) = self.lookup.get(s) {
            return id;
        }
        let id = InternedString(self.strings.len() as u32);
        self.strings.push(s.to_string());
        self.lookup.insert(s.to_string(), id);
        id
    }

    /// Resolve an interned string handle back to its string.
    pub fn resolve(&self, id: InternedString) -> &str {
        &self.strings[id.0 as usize]
    }

    /// Number of unique strings interned.
    pub fn len(&self) -> usize {
        self.strings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }
}

impl Default for StringInterner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intern_and_resolve() {
        let mut interner = StringInterner::new();
        let a = interner.intern("hello");
        let b = interner.intern("world");
        let c = interner.intern("hello"); // duplicate

        assert_eq!(a, c); // same string → same handle
        assert_ne!(a, b);
        assert_eq!(interner.resolve(a), "hello");
        assert_eq!(interner.resolve(b), "world");
        assert_eq!(interner.len(), 2);
    }
}
