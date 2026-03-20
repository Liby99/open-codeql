use ordered_float::OrderedFloat;

use crate::InternedString;

/// Entity ID (used for @-prefixed database types).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityId(pub u64);

/// A single value in the database.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Value {
    Int(i64),
    Float(OrderedFloat<f64>),
    String(InternedString),
    Bool(bool),
    Entity(EntityId),
    Null,
}

impl Value {
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_entity(&self) -> Option<EntityId> {
        match self {
            Value::Entity(id) => Some(*id),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<InternedString> {
        match self {
            Value::String(s) => Some(*s),
            _ => None,
        }
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Float(OrderedFloat(v))
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}

impl From<EntityId> for Value {
    fn from(v: EntityId) -> Self {
        Value::Entity(v)
    }
}

impl From<InternedString> for Value {
    fn from(v: InternedString) -> Self {
        Value::String(v)
    }
}
