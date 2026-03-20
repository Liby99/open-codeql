use ocql_common::Span;

use crate::LowerName;

/// An annotation on a QL declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub kind: AnnotationKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AnnotationKind {
    /// `abstract`
    Abstract,
    /// `cached`
    Cached,
    /// `external`
    External,
    /// `extensible`
    Extensible,
    /// `transient`
    Transient,
    /// `final`
    Final,
    /// `override`
    Override,
    /// `deprecated`
    Deprecated,
    /// `query`
    Query,
    /// `additional`
    Additional,
    /// `private`
    Private,
    /// `library`
    Library,
    /// `default`
    Default,
    /// `signature`
    Signature,

    /// `pragma[name]`
    Pragma(PragmaKind),

    /// `bindingset[x, y, ...]`
    BindingSet(Vec<LowerName>),

    /// `language[monotonicAggregates]`
    Language(LanguageFeature),

    /// `overlay[local]` or `overlay[global]`
    Overlay(OverlayKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OverlayKind {
    Local,
    Global,
    DiscardEntity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PragmaKind {
    Inline,
    InlineLate,
    NoInline,
    NoMagic,
    NoOpt,
    OnlyBindOut,
    OnlyBindInto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LanguageFeature {
    MonotonicAggregates,
}
