//! Access path parsing for CodeQL model specifications.
//!
//! Access paths describe where in a value's structure taint exists or flows.
//! Examples:
//!   - `Argument[0]`
//!   - `Argument[this]`
//!   - `ReturnValue`
//!   - `Argument[0].Field[name].MapValue`
//!   - `Argument[this].SyntheticField[android.content.Intent.extras].MapKey`

/// A parsed access path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccessPath {
    pub root: AccessPathRoot,
    pub components: Vec<AccessPathComponent>,
}

/// The root of an access path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AccessPathRoot {
    /// `Argument[N]` — positional argument
    Argument(ArgumentSpec),
    /// `ReturnValue` — the method return value
    ReturnValue(Option<String>),
    /// Empty string — no specific path
    Empty,
}

/// Specifies which argument(s).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ArgumentSpec {
    /// `Argument[N]`
    Index(i32),
    /// `Argument[this]`
    This,
    /// `Argument[N..M]` — inclusive range
    Range(i32, i32),
    /// `Argument[*N]` — dereferenced argument
    Deref(i32),
    /// `Argument[**N]` — double-dereferenced argument
    DerefDeref(i32),
    /// `Argument[*@N]` — dereference+element argument (C++)
    DerefElement(i32),
    /// `Argument[*]` — all arguments
    All,
}

/// A component in the access path after the root.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AccessPathComponent {
    /// `.Field[name]`
    Field(String),
    /// `.Field[*name]` — pointer-to-field dereference
    DerefField(String),
    /// `.SyntheticField[name]`
    SyntheticField(String),
    /// `.MapKey`
    MapKey,
    /// `.MapValue`
    MapValue,
    /// `.ArrayElement`
    ArrayElement,
    /// `.Element[@]` — generic collection element
    Element,
}

/// Parse an access path string.
pub fn parse_access_path(s: &str) -> Result<AccessPath, String> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(AccessPath {
            root: AccessPathRoot::Empty,
            components: vec![],
        });
    }

    let (root, rest) = parse_root(s)?;
    let components = parse_components(rest)?;

    Ok(AccessPath { root, components })
}

fn parse_root(s: &str) -> Result<(AccessPathRoot, &str), String> {
    if s.starts_with("Argument[") || s.starts_with("Parameter[") {
        let bracket = s.find('[').unwrap();
        let close = s.find(']').ok_or("unclosed bracket in Argument/Parameter")?;
        let spec_str = &s[bracket + 1..close];
        let spec = parse_argument_spec(spec_str)?;
        let rest = &s[close + 1..];
        Ok((AccessPathRoot::Argument(spec), rest))
    } else if s.starts_with("ReturnValue") {
        let rest = &s[11..];
        if rest.starts_with('[') {
            let close = rest.find(']').ok_or("unclosed bracket in ReturnValue")?;
            let qualifier = rest[1..close].to_string();
            Ok((AccessPathRoot::ReturnValue(Some(qualifier)), &rest[close + 1..]))
        } else {
            Ok((AccessPathRoot::ReturnValue(None), rest))
        }
    } else {
        Err(format!("unexpected access path root: {}", s))
    }
}

fn parse_argument_spec(s: &str) -> Result<ArgumentSpec, String> {
    if s == "this" {
        Ok(ArgumentSpec::This)
    } else if s == "*" {
        Ok(ArgumentSpec::All)
    } else if let Some(rest) = s.strip_prefix("**") {
        let n: i32 = rest.parse().map_err(|e| format!("invalid deref-deref index: {}", e))?;
        Ok(ArgumentSpec::DerefDeref(n))
    } else if let Some(rest) = s.strip_prefix("*@") {
        let n: i32 = rest.parse().map_err(|e| format!("invalid deref-element index: {}", e))?;
        Ok(ArgumentSpec::DerefElement(n))
    } else if let Some(rest) = s.strip_prefix('*') {
        let n: i32 = rest.parse().map_err(|e| format!("invalid deref index: {}", e))?;
        Ok(ArgumentSpec::Deref(n))
    } else if s.contains("..") {
        let parts: Vec<&str> = s.split("..").collect();
        if parts.len() != 2 {
            return Err(format!("invalid range: {}", s));
        }
        let lo: i32 = parts[0].parse().map_err(|e| format!("invalid range start: {}", e))?;
        let hi: i32 = parts[1].parse().map_err(|e| format!("invalid range end: {}", e))?;
        Ok(ArgumentSpec::Range(lo, hi))
    } else {
        let n: i32 = s.parse().map_err(|e| format!("invalid argument index '{}': {}", s, e))?;
        Ok(ArgumentSpec::Index(n))
    }
}

fn parse_components(mut s: &str) -> Result<Vec<AccessPathComponent>, String> {
    let mut components = Vec::new();

    while !s.is_empty() {
        if !s.starts_with('.') {
            return Err(format!("expected '.' before component, got: {}", s));
        }
        s = &s[1..]; // skip '.'

        if let Some(rest) = s.strip_prefix("SyntheticField[") {
            let close = rest.find(']').ok_or("unclosed SyntheticField bracket")?;
            components.push(AccessPathComponent::SyntheticField(rest[..close].to_string()));
            s = &rest[close + 1..];
        } else if let Some(rest) = s.strip_prefix("Field[") {
            let close = rest.find(']').ok_or("unclosed Field bracket")?;
            let name = &rest[..close];
            if let Some(deref_name) = name.strip_prefix('*') {
                components.push(AccessPathComponent::DerefField(deref_name.to_string()));
            } else {
                components.push(AccessPathComponent::Field(name.to_string()));
            }
            s = &rest[close + 1..];
        } else if s.starts_with("MapKey") {
            components.push(AccessPathComponent::MapKey);
            s = &s[6..];
        } else if s.starts_with("MapValue") {
            components.push(AccessPathComponent::MapValue);
            s = &s[8..];
        } else if s.starts_with("ArrayElement") {
            components.push(AccessPathComponent::ArrayElement);
            s = &s[12..];
        } else if s.starts_with("Element[@]") {
            components.push(AccessPathComponent::Element);
            s = &s[10..];
        } else {
            return Err(format!("unknown component: .{}", s));
        }
    }

    Ok(components)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty() {
        let ap = parse_access_path("").unwrap();
        assert_eq!(ap.root, AccessPathRoot::Empty);
        assert!(ap.components.is_empty());
    }

    #[test]
    fn parse_argument_index() {
        let ap = parse_access_path("Argument[0]").unwrap();
        assert_eq!(ap.root, AccessPathRoot::Argument(ArgumentSpec::Index(0)));
    }

    #[test]
    fn parse_argument_this() {
        let ap = parse_access_path("Argument[this]").unwrap();
        assert_eq!(ap.root, AccessPathRoot::Argument(ArgumentSpec::This));
    }

    #[test]
    fn parse_argument_negative() {
        let ap = parse_access_path("Argument[-1]").unwrap();
        assert_eq!(ap.root, AccessPathRoot::Argument(ArgumentSpec::Index(-1)));
    }

    #[test]
    fn parse_argument_range() {
        let ap = parse_access_path("Argument[0..2]").unwrap();
        assert_eq!(ap.root, AccessPathRoot::Argument(ArgumentSpec::Range(0, 2)));
    }

    #[test]
    fn parse_argument_deref() {
        let ap = parse_access_path("Argument[*1]").unwrap();
        assert_eq!(ap.root, AccessPathRoot::Argument(ArgumentSpec::Deref(1)));
    }

    #[test]
    fn parse_argument_all() {
        let ap = parse_access_path("Argument[*]").unwrap();
        assert_eq!(ap.root, AccessPathRoot::Argument(ArgumentSpec::All));
    }

    #[test]
    fn parse_return_value() {
        let ap = parse_access_path("ReturnValue").unwrap();
        assert_eq!(ap.root, AccessPathRoot::ReturnValue(None));
    }

    #[test]
    fn parse_return_value_qualified() {
        let ap = parse_access_path("ReturnValue[*]").unwrap();
        assert_eq!(ap.root, AccessPathRoot::ReturnValue(Some("*".into())));
    }

    #[test]
    fn parse_field_component() {
        let ap = parse_access_path("Argument[this].Field[data]").unwrap();
        assert_eq!(ap.root, AccessPathRoot::Argument(ArgumentSpec::This));
        assert_eq!(ap.components, vec![AccessPathComponent::Field("data".into())]);
    }

    #[test]
    fn parse_deref_field() {
        let ap = parse_access_path("Argument[0].Field[*pvData]").unwrap();
        assert_eq!(ap.components, vec![AccessPathComponent::DerefField("pvData".into())]);
    }

    #[test]
    fn parse_synthetic_field() {
        let ap = parse_access_path("Argument[this].SyntheticField[android.content.Intent.extras]").unwrap();
        assert_eq!(ap.root, AccessPathRoot::Argument(ArgumentSpec::This));
        assert_eq!(ap.components, vec![
            AccessPathComponent::SyntheticField("android.content.Intent.extras".into()),
        ]);
    }

    #[test]
    fn parse_map_key_value() {
        let ap = parse_access_path("Argument[0].MapKey").unwrap();
        assert_eq!(ap.components, vec![AccessPathComponent::MapKey]);

        let ap = parse_access_path("Argument[this].MapValue").unwrap();
        assert_eq!(ap.components, vec![AccessPathComponent::MapValue]);
    }

    #[test]
    fn parse_array_element() {
        let ap = parse_access_path("Argument[0].ArrayElement").unwrap();
        assert_eq!(ap.components, vec![AccessPathComponent::ArrayElement]);
    }

    #[test]
    fn parse_element() {
        let ap = parse_access_path("Argument[-1].Element[@]").unwrap();
        assert_eq!(ap.root, AccessPathRoot::Argument(ArgumentSpec::Index(-1)));
        assert_eq!(ap.components, vec![AccessPathComponent::Element]);
    }

    #[test]
    fn parse_chained_components() {
        let ap = parse_access_path(
            "Argument[this].SyntheticField[android.content.Intent.extras].MapValue"
        ).unwrap();
        assert_eq!(ap.root, AccessPathRoot::Argument(ArgumentSpec::This));
        assert_eq!(ap.components, vec![
            AccessPathComponent::SyntheticField("android.content.Intent.extras".into()),
            AccessPathComponent::MapValue,
        ]);
    }
}
