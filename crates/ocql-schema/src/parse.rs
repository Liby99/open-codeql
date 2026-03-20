use crate::types::*;

/// Error type for .dbscheme parsing.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.col, self.message)
    }
}

impl std::error::Error for ParseError {}

/// Parse a .dbscheme file into a `DbScheme`.
pub fn parse_dbscheme(input: &str) -> Result<DbScheme, ParseError> {
    let mut parser = Parser::new(input);
    parser.parse_file()
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn remaining(&self) -> &'a str {
        &self.input[self.pos..]
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn line_col(&self) -> (usize, usize) {
        let consumed = &self.input[..self.pos];
        let line = consumed.chars().filter(|&c| c == '\n').count() + 1;
        let col = match consumed.rfind('\n') {
            Some(nl) => self.pos - nl,
            None => self.pos + 1,
        };
        (line, col)
    }

    fn error(&self, msg: impl Into<String>) -> ParseError {
        let (line, col) = self.line_col();
        ParseError { message: msg.into(), line, col }
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            // Skip whitespace
            while self.pos < self.input.len() {
                let b = self.input.as_bytes()[self.pos];
                if b == b' ' || b == b'\t' || b == b'\r' || b == b'\n' {
                    self.pos += 1;
                } else {
                    break;
                }
            }

            let rem = self.remaining();

            // Skip line comments
            if rem.starts_with("//") {
                if let Some(nl) = rem.find('\n') {
                    self.pos += nl + 1;
                    continue;
                } else {
                    self.pos = self.input.len();
                    return;
                }
            }

            // Skip block comments (handles nested /* inside /** ... */ QLDoc)
            if rem.starts_with("/*") {
                let start = self.pos;
                self.pos += 2;
                let mut depth = 1;
                while self.pos < self.input.len() && depth > 0 {
                    if self.remaining().starts_with("/*") {
                        depth += 1;
                        self.pos += 2;
                    } else if self.remaining().starts_with("*/") {
                        depth -= 1;
                        self.pos += 2;
                    } else {
                        self.pos += 1;
                    }
                }
                if depth != 0 {
                    // Unterminated comment — reset to just skip the `/*`
                    self.pos = start + 2;
                }
                continue;
            }

            break;
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    fn expect_char(&mut self, ch: char) -> Result<(), ParseError> {
        self.skip_whitespace_and_comments();
        if self.peek_char() == Some(ch) {
            self.pos += ch.len_utf8();
            Ok(())
        } else {
            Err(self.error(format!("expected '{}', found {:?}", ch, self.peek_char())))
        }
    }

    fn try_char(&mut self, ch: char) -> bool {
        self.skip_whitespace_and_comments();
        if self.peek_char() == Some(ch) {
            self.pos += ch.len_utf8();
            true
        } else {
            false
        }
    }

    fn expect_str(&mut self, s: &str) -> Result<(), ParseError> {
        self.skip_whitespace_and_comments();
        if self.remaining().starts_with(s) {
            // Make sure it's not a prefix of a longer identifier
            let after = self.input.as_bytes().get(self.pos + s.len());
            if let Some(&b) = after {
                if b.is_ascii_alphanumeric() || b == b'_' {
                    return Err(self.error(format!("expected keyword '{}'", s)));
                }
            }
            self.pos += s.len();
            Ok(())
        } else {
            Err(self.error(format!("expected '{}'", s)))
        }
    }

    /// Parse an identifier: [a-zA-Z_][a-zA-Z0-9_]*
    fn parse_ident(&mut self) -> Result<String, ParseError> {
        self.skip_whitespace_and_comments();
        let start = self.pos;
        while self.pos < self.input.len() {
            let b = self.input.as_bytes()[self.pos];
            if b.is_ascii_alphanumeric() || b == b'_' {
                self.pos += 1;
            } else {
                break;
            }
        }
        if self.pos == start {
            return Err(self.error("expected identifier"));
        }
        Ok(self.input[start..self.pos].to_string())
    }

    /// Parse a db-type reference: @identifier
    fn parse_at_type(&mut self) -> Result<String, ParseError> {
        self.skip_whitespace_and_comments();
        if self.peek_char() != Some('@') {
            return Err(self.error("expected '@' type"));
        }
        let start = self.pos;
        self.pos += 1; // skip @
        while self.pos < self.input.len() {
            let b = self.input.as_bytes()[self.pos];
            if b.is_ascii_alphanumeric() || b == b'_' {
                self.pos += 1;
            } else {
                break;
            }
        }
        Ok(self.input[start..self.pos].to_string())
    }

    /// Parse an integer literal (possibly negative).
    fn parse_int(&mut self) -> Result<i64, ParseError> {
        self.skip_whitespace_and_comments();
        let start = self.pos;
        if self.peek_char() == Some('-') {
            self.pos += 1;
        }
        while self.pos < self.input.len() && self.input.as_bytes()[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        if self.pos == start || (self.pos == start + 1 && self.input.as_bytes()[start] == b'-') {
            return Err(self.error("expected integer"));
        }
        self.input[start..self.pos]
            .parse::<i64>()
            .map_err(|e| self.error(format!("invalid integer: {}", e)))
    }

    fn parse_file(&mut self) -> Result<DbScheme, ParseError> {
        let mut entries = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            if self.is_eof() {
                break;
            }

            match self.parse_entry() {
                Ok(entry) => entries.push(entry),
                Err(e) => return Err(e),
            }
        }
        Ok(DbScheme { entries })
    }

    fn parse_entry(&mut self) -> Result<Entry, ParseError> {
        self.skip_whitespace_and_comments();

        // Check what we're looking at
        let rem = self.remaining();

        // Case block: `case @type.col of`
        if rem.starts_with("case") && !rem[4..5].chars().next().map_or(false, |c| c.is_ascii_alphanumeric() || c == '_') {
            return self.parse_case().map(Entry::Case);
        }

        // Keyset + table or just table
        if rem.starts_with('#') {
            return self.parse_table_with_keysets().map(Entry::Table);
        }

        // Union: `@name = @a | @b`
        if rem.starts_with('@') {
            return self.parse_union().map(Entry::Union);
        }

        // Otherwise it's a table (starting with a lowercase identifier)
        self.parse_table_with_keysets().map(Entry::Table)
    }

    fn parse_table_with_keysets(&mut self) -> Result<Table, ParseError> {
        let mut keysets = Vec::new();

        // Parse any #keyset annotations (possibly multiple on one line)
        while self.remaining().starts_with('#') {
            keysets.push(self.parse_keyset()?);
            self.skip_whitespace_and_comments();
        }

        // Parse the table
        let mut table = self.parse_table()?;
        table.keysets = keysets;
        Ok(table)
    }

    fn parse_keyset(&mut self) -> Result<Vec<String>, ParseError> {
        self.expect_char('#')?;
        self.expect_str("keyset")?;
        self.expect_char('[')?;
        let mut cols = vec![self.parse_ident()?];
        while self.try_char(',') {
            cols.push(self.parse_ident()?);
        }
        self.expect_char(']')?;
        Ok(cols)
    }

    fn parse_table(&mut self) -> Result<Table, ParseError> {
        let name = self.parse_ident()?;
        self.expect_char('(')?;

        let mut columns = Vec::new();
        // Parse columns (comma-separated, may have QLDoc comments)
        loop {
            self.skip_whitespace_and_comments();
            if self.peek_char() == Some(')') {
                break;
            }
            if !columns.is_empty() {
                self.expect_char(',')?;
                self.skip_whitespace_and_comments();
                if self.peek_char() == Some(')') {
                    break; // trailing comma
                }
            }
            columns.push(self.parse_column()?);
        }

        self.expect_char(')')?;
        // Optional semicolon
        self.try_char(';');

        Ok(Table { name, columns, keysets: Vec::new() })
    }

    fn parse_column(&mut self) -> Result<Column, ParseError> {
        self.skip_whitespace_and_comments();

        // Optional `unique`
        let is_unique = if self.remaining().starts_with("unique")
            && !self.input.as_bytes().get(self.pos + 6).map_or(false, |b| b.is_ascii_alphanumeric() || *b == b'_')
        {
            self.pos += 6;
            true
        } else {
            false
        };

        // Column QL type (int, float, string, boolean, date)
        let col_type = self.parse_column_type()?;

        // Column name
        let name = self.parse_ident()?;

        // `:` separator
        self.expect_char(':')?;

        // Database type (after the colon)
        let db_type = self.parse_db_type()?;

        // Optional `ref`
        self.skip_whitespace_and_comments();
        let is_ref = if self.remaining().starts_with("ref")
            && !self.input.as_bytes().get(self.pos + 3).map_or(false, |b| b.is_ascii_alphanumeric() || *b == b'_')
        {
            self.pos += 3;
            true
        } else {
            false
        };

        Ok(Column { name, col_type, db_type, is_unique, is_ref })
    }

    fn parse_column_type(&mut self) -> Result<ColumnType, ParseError> {
        self.skip_whitespace_and_comments();
        let rem = self.remaining();

        // varchar(N)
        if rem.starts_with("varchar") {
            self.pos += 7;
            self.expect_char('(')?;
            let n = self.parse_int()? as u32;
            self.expect_char(')')?;
            return Ok(ColumnType::Varchar(n));
        }

        let types = [
            ("boolean", ColumnType::Boolean),
            ("float", ColumnType::Float),
            ("string", ColumnType::String),
            ("date", ColumnType::Date),
            ("int", ColumnType::Int),
        ];
        for (kw, ty) in &types {
            if rem.starts_with(kw)
                && !self.input.as_bytes().get(self.pos + kw.len()).map_or(false, |b| b.is_ascii_alphanumeric() || *b == b'_')
            {
                self.pos += kw.len();
                return Ok(ty.clone());
            }
        }
        Err(self.error(format!("expected column type (int/float/string/boolean/date/varchar), found {:?}", &rem[..rem.len().min(20)])))
    }

    fn parse_db_type(&mut self) -> Result<DbType, ParseError> {
        self.skip_whitespace_and_comments();
        let rem = self.remaining();

        // Entity type
        if rem.starts_with('@') {
            let entity = self.parse_at_type()?;
            return Ok(DbType::Entity(entity));
        }

        // Primitive types
        let types = [
            ("boolean", DbType::Boolean),
            ("float", DbType::Float),
            ("string", DbType::String),
            ("date", DbType::Date),
            ("int", DbType::Int),
        ];
        for (kw, ty) in &types {
            if rem.starts_with(kw)
                && !self.input.as_bytes().get(self.pos + kw.len()).map_or(false, |b| b.is_ascii_alphanumeric() || *b == b'_')
            {
                self.pos += kw.len();
                return Ok(ty.clone());
            }
        }
        Err(self.error(format!("expected db type, found {:?}", &rem[..rem.len().min(20)])))
    }

    fn parse_union(&mut self) -> Result<UnionType, ParseError> {
        let name = self.parse_at_type()?;
        self.expect_char('=')?;

        let mut variants = vec![self.parse_at_type()?];
        while self.try_char('|') {
            variants.push(self.parse_at_type()?);
        }

        // Optional semicolon
        self.try_char(';');

        Ok(UnionType { name, variants })
    }

    fn parse_case(&mut self) -> Result<CaseType, ParseError> {
        self.expect_str("case")?;

        // @entity.column
        let entity = self.parse_at_type()?;
        self.expect_char('.')?;
        let column = self.parse_ident()?;

        self.expect_str("of")?;

        // Parse variants: [|] int = @type
        let mut variants = Vec::new();

        // First variant (no leading `|`)
        self.skip_whitespace_and_comments();
        if self.peek_char() != Some(';') {
            let value = self.parse_int()?;
            self.expect_char('=')?;
            let entity_type = self.parse_at_type()?;
            variants.push(CaseVariant { value, entity_type });
        }

        // Remaining variants (with leading `|`)
        while self.try_char('|') {
            let value = self.parse_int()?;
            self.expect_char('=')?;
            let entity_type = self.parse_at_type()?;
            variants.push(CaseVariant { value, entity_type });
        }

        self.expect_char(';')?;

        Ok(CaseType { entity, column, variants })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_table() {
        let input = r#"
            files(
                unique int id: @file,
                string name: string ref
            );
        "#;
        let result = parse_dbscheme(input).unwrap();
        assert_eq!(result.entries.len(), 1);
        let table = result.tables().next().unwrap();
        assert_eq!(table.name, "files");
        assert_eq!(table.columns.len(), 2);
        assert!(table.columns[0].is_unique);
        assert_eq!(table.columns[0].name, "id");
        assert_eq!(table.columns[0].db_type, DbType::Entity("@file".into()));
        assert!(!table.columns[0].is_ref);
        assert!(table.columns[1].is_ref);
    }

    #[test]
    fn test_parse_table_no_semicolon() {
        let input = r#"
            compilation_started(
                int id : @compilation ref
            )
        "#;
        let result = parse_dbscheme(input).unwrap();
        assert_eq!(result.entries.len(), 1);
    }

    #[test]
    fn test_parse_keyset() {
        let input = r#"
            #keyset[id, num]
            compilation_args(
                int id : @compilation ref,
                int num : int ref,
                string arg : string ref
            );
        "#;
        let result = parse_dbscheme(input).unwrap();
        let table = result.tables().next().unwrap();
        assert_eq!(table.keysets.len(), 1);
        assert_eq!(table.keysets[0], vec!["id", "num"]);
    }

    #[test]
    fn test_parse_multiple_keysets() {
        let input = r#"
            #keyset[fieldId] #keyset[fieldDeclId,pos]
            fieldDeclaredIn(
                int fieldId: @field ref,
                int fieldDeclId: @fielddecl ref,
                int pos: int ref
            );
        "#;
        let result = parse_dbscheme(input).unwrap();
        let table = result.tables().next().unwrap();
        assert_eq!(table.keysets.len(), 2);
    }

    #[test]
    fn test_parse_union() {
        let input = "@container = @file | @folder";
        let result = parse_dbscheme(input).unwrap();
        let union = result.unions().next().unwrap();
        assert_eq!(union.name, "@container");
        assert_eq!(union.variants, vec!["@file", "@folder"]);
    }

    #[test]
    fn test_parse_union_with_semicolon() {
        let input = "@stmtparent = @callable | @stmt | @switchexpr;";
        let result = parse_dbscheme(input).unwrap();
        let union = result.unions().next().unwrap();
        assert_eq!(union.name, "@stmtparent");
        assert_eq!(union.variants.len(), 3);
    }

    #[test]
    fn test_parse_case() {
        let input = r#"
            case @compilation.kind of
               1  = @javacompilation
            |  2  = @kotlincompilation
            ;
        "#;
        let result = parse_dbscheme(input).unwrap();
        let case = result.cases().next().unwrap();
        assert_eq!(case.entity, "@compilation");
        assert_eq!(case.column, "kind");
        assert_eq!(case.variants.len(), 2);
        assert_eq!(case.variants[0].value, 1);
        assert_eq!(case.variants[0].entity_type, "@javacompilation");
        assert_eq!(case.variants[1].value, 2);
        assert_eq!(case.variants[1].entity_type, "@kotlincompilation");
    }

    #[test]
    fn test_parse_mixed() {
        let input = r#"
            files(
                unique int id: @file,
                string name: string ref
            );

            folders(
                unique int id: @folder,
                string name: string ref
            );

            @container = @file | @folder

            case @macroinvocation.kind of
              1 = @macro_expansion
            | 2 = @other_macro_reference
            ;
        "#;
        let result = parse_dbscheme(input).unwrap();
        assert_eq!(result.tables().count(), 2);
        assert_eq!(result.unions().count(), 1);
        assert_eq!(result.cases().count(), 1);
    }

    #[test]
    fn test_parse_with_comments() {
        let input = r#"
            /** Documentation comment */
            files(
                /** The file id */
                unique int id: @file,
                // line comment
                string name: string ref
            );
        "#;
        let result = parse_dbscheme(input).unwrap();
        assert_eq!(result.entries.len(), 1);
    }

    #[test]
    fn test_parse_cpp_dbscheme() {
        let content = std::fs::read_to_string(
            "../../vendor/codeql/cpp/ql/lib/semmlecode.cpp.dbscheme"
        );
        if let Ok(content) = content {
            let result = parse_dbscheme(&content);
            assert!(result.is_ok(), "Parse error: {}", result.err().unwrap());
            let schema = result.unwrap();
            let table_count = schema.tables().count();
            let union_count = schema.unions().count();
            let case_count = schema.cases().count();
            eprintln!("C++ dbscheme: {} tables, {} unions, {} cases", table_count, union_count, case_count);
            assert!(table_count > 200, "Expected >200 tables, got {}", table_count);
            assert!(union_count > 10, "Expected >10 unions, got {}", union_count);
            assert!(case_count > 5, "Expected >5 cases, got {}", case_count);
        }
    }

    #[test]
    fn test_parse_java_dbscheme() {
        let content = std::fs::read_to_string(
            "../../vendor/codeql/java/ql/lib/config/semmlecode.dbscheme"
        );
        if let Ok(content) = content {
            let result = parse_dbscheme(&content);
            assert!(result.is_ok(), "Parse error: {}", result.err().unwrap());
            let schema = result.unwrap();
            let table_count = schema.tables().count();
            let union_count = schema.unions().count();
            let case_count = schema.cases().count();
            eprintln!("Java dbscheme: {} tables, {} unions, {} cases", table_count, union_count, case_count);
            assert!(table_count > 50, "Expected >50 tables, got {}", table_count);
        }
    }

    fn parse_vendor_dbscheme(path: &str) -> Option<DbScheme> {
        let content = std::fs::read_to_string(path).ok()?;
        let result = parse_dbscheme(&content);
        assert!(result.is_ok(), "Parse error in {}: {}", path, result.err().unwrap());
        Some(result.unwrap())
    }

    #[test]
    fn test_parse_csharp_dbscheme() {
        if let Some(schema) = parse_vendor_dbscheme("../../vendor/codeql/csharp/ql/lib/semmlecode.csharp.dbscheme") {
            eprintln!("C# dbscheme: {} tables, {} unions, {} cases",
                schema.tables().count(), schema.unions().count(), schema.cases().count());
            assert!(schema.tables().count() > 50);
        }
    }

    #[test]
    fn test_parse_python_dbscheme() {
        if let Some(schema) = parse_vendor_dbscheme("../../vendor/codeql/python/ql/lib/semmlecode.python.dbscheme") {
            eprintln!("Python dbscheme: {} tables, {} unions, {} cases",
                schema.tables().count(), schema.unions().count(), schema.cases().count());
            assert!(schema.tables().count() > 20);
        }
    }

    #[test]
    fn test_parse_javascript_dbscheme() {
        if let Some(schema) = parse_vendor_dbscheme("../../vendor/codeql/javascript/ql/lib/semmlecode.javascript.dbscheme") {
            eprintln!("JavaScript dbscheme: {} tables, {} unions, {} cases",
                schema.tables().count(), schema.unions().count(), schema.cases().count());
            assert!(schema.tables().count() > 20);
        }
    }

    #[test]
    fn test_parse_ruby_dbscheme() {
        if let Some(schema) = parse_vendor_dbscheme("../../vendor/codeql/ruby/ql/lib/ruby.dbscheme") {
            eprintln!("Ruby dbscheme: {} tables, {} unions, {} cases",
                schema.tables().count(), schema.unions().count(), schema.cases().count());
            assert!(schema.tables().count() > 10);
        }
    }

    #[test]
    fn test_parse_go_dbscheme() {
        if let Some(schema) = parse_vendor_dbscheme("../../vendor/codeql/go/ql/lib/go.dbscheme") {
            eprintln!("Go dbscheme: {} tables, {} unions, {} cases",
                schema.tables().count(), schema.unions().count(), schema.cases().count());
            assert!(schema.tables().count() > 20);
        }
    }

    #[test]
    fn test_parse_swift_dbscheme() {
        if let Some(schema) = parse_vendor_dbscheme("../../vendor/codeql/swift/ql/lib/swift.dbscheme") {
            eprintln!("Swift dbscheme: {} tables, {} unions, {} cases",
                schema.tables().count(), schema.unions().count(), schema.cases().count());
            assert!(schema.tables().count() > 10);
        }
    }
}
