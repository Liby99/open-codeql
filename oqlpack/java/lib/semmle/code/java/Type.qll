/**
 * Provides classes and predicates for working with Java types.
 *
 * Types can be primitive types (`PrimitiveType`), array types (`Array`), or reference
 * types (`RefType`), where the latter are either classes (`Class`) or interfaces
 * (`Interface`).
 */

import Member
import Modifier

/**
 * Holds if reference type `t` is an immediate super-type of `sub`.
 */
predicate hasSubtype(RefType t, Type sub) {
  extendsReftype(sub, t) and t != sub
  or
  implInterface(sub, t)
}

/**
 * Holds if reference type `anc` is a direct or indirect supertype of `sub`, including itself.
 */
predicate hasDescendant(RefType anc, Type sub) {
  anc = sub
  or
  exists(RefType mid | hasSubtype(anc, mid) and hasDescendant(mid, sub))
}

/** A type. */
class Type extends Element, @type {
  /** Gets the package in which this type is declared. */
  Package getPackage() { none() }

  /** Gets the erasure of this type. */
  Type getErasure() { result = this }

  override string getAPrimaryQlClass() { result = "Type" }
}

/** An array type. */
class Array extends RefType, @array {
  /** Gets the component type of this array type. */
  Type getComponentType() { arrays(this, _, _, _, result) }

  /** Gets the element type of this array type. */
  Type getElementType() { arrays(this, _, result, _, _) }

  /** Gets the dimension of this array type. */
  int getDimension() { arrays(this, _, _, result, _) }

  override string getAPrimaryQlClass() { result = "Array" }
}

/** A reference type (class, interface, array, or type variable). */
class RefType extends Type, Annotatable, Modifiable, @reftype {
  /** Gets the package in which this type is declared. */
  override Package getPackage() {
    classes_or_interfaces(this, _, result, _)
  }

  /** Gets the source declaration of this type. */
  RefType getSourceDeclaration() {
    classes_or_interfaces(this, _, _, result)
  }

  /** Holds if this type is the same as its source declaration. */
  predicate isSourceDeclaration() { this.getSourceDeclaration() = this }

  /** Gets a method declared in this type. */
  Method getAMethod() { methods(result, _, _, _, this, _) }

  /** Gets a constructor declared in this type. */
  Constructor getAConstructor() { constrs(result, _, _, _, this, _) }

  /** Gets a member declared in this type. */
  Member getAMember() { declaresMember(this, result) }

  /** Gets a field declared in this type. */
  Field getAField() { fields(result, _, _, this) }

  /** Gets a callable (method or constructor) declared in this type. */
  Callable getACallable() {
    result = this.getAMethod() or result = this.getAConstructor()
  }

  /** Holds if this type extends `t`. */
  predicate extendsOrImplements(RefType t) {
    extendsReftype(this, t) or implInterface(this, t)
  }

  /** Gets a direct supertype of this type. */
  RefType getASupertype() { this.extendsOrImplements(result) }

  /** Gets a direct or indirect supertype of this type, including itself. */
  RefType getAnAncestor() { hasDescendant(result, this) }

  /**
   * Gets the number of parameters of this type.
   */
  int getNumberOfTypeParameters() { result = count(int i | typeVars(_, _, i, this)) }

  /** Gets the enclosing type, if this is a nested type. */
  RefType getEnclosingType() { enclInReftype(this, result) }

  /** Holds if this is a top-level type (not nested). */
  predicate isTopLevel() { not enclInReftype(this, _) }

  override string getAPrimaryQlClass() { result = "RefType" }
}

/** A class or interface. */
class ClassOrInterface extends RefType, @classorinterface {
  /** Holds if this is a local class. */
  predicate isLocal() { this instanceof LocalClassOrInterface }

  /** Holds if this is an anonymous class. */
  predicate isAnonymous() { this instanceof AnonymousClass }

  /** Holds if this type is sealed. */
  predicate isSealed() { none() }

  override string getAPrimaryQlClass() { result = "ClassOrInterface" }
}

/** A class (not an interface). */
class Class extends ClassOrInterface {
  Class() { not isInterface(this) }

  override string getAPrimaryQlClass() { result = "Class" }
}

/** An interface. */
class Interface extends ClassOrInterface {
  Interface() { isInterface(this) }

  override string getAPrimaryQlClass() { result = "Interface" }
}

/** A type that is source-available. */
class SrcRefType extends RefType {
  SrcRefType() { this.fromSource() }
}

/** A top-level type (not nested). */
class TopLevelType extends RefType {
  TopLevelType() { this.isTopLevel() }
}

/** A top-level class. */
class TopLevelClass extends TopLevelType, Class { }

/** A nested type. */
class NestedType extends RefType {
  NestedType() { enclInReftype(this, _) }

  /** Gets the immediately enclosing type. */
  RefType getEnclosingType() { enclInReftype(this, result) }
}

/** A nested class. */
class NestedClass extends NestedType, Class { }

/** An inner class (non-static nested class). */
class InnerClass extends NestedClass {
  InnerClass() { not this.isStatic() }
}

/** A local class or interface. */
class LocalClassOrInterface extends NestedType, ClassOrInterface {
  LocalClassOrInterface() {
    exists(Stmt s | s.getEnclosingCallable() = _ and enclInReftype(this, _))
  }
}

/** A local class. */
class LocalClass extends LocalClassOrInterface, NestedClass { }

/** An anonymous class. */
class AnonymousClass extends NestedClass {
  AnonymousClass() { this.getName() = "" }
}

/** A member type. */
class MemberType extends NestedType, Member { }

/** A primitive type such as `int`, `boolean`, or `void`. */
class PrimitiveType extends Type, @primitive {
  override string getAPrimaryQlClass() { result = "PrimitiveType" }
}

/** The `void` type. */
class VoidType extends Type, @primitive {
  VoidType() { this.hasName("void") }
}

/** The `null` type. */
class NullType extends Type, @primitive {
  NullType() { this.hasName("<nulltype>") }
}

/** A boxed type such as `Integer` or `Boolean`. */
class BoxedType extends RefType {
  BoxedType() {
    exists(string name | this.hasName(name) |
      name = "Boolean" or name = "Byte" or name = "Character" or
      name = "Short" or name = "Integer" or name = "Long" or
      name = "Float" or name = "Double"
    ) and
    this.getPackage().hasName("java.lang")
  }

  /** Gets the primitive type that this boxes. */
  PrimitiveType getPrimitiveType() {
    this.hasName("Boolean") and result.hasName("boolean") or
    this.hasName("Byte") and result.hasName("byte") or
    this.hasName("Character") and result.hasName("char") or
    this.hasName("Short") and result.hasName("short") or
    this.hasName("Integer") and result.hasName("int") or
    this.hasName("Long") and result.hasName("long") or
    this.hasName("Float") and result.hasName("float") or
    this.hasName("Double") and result.hasName("double")
  }
}

/** An enum type. */
class EnumType extends Class {
  EnumType() { isEnumType(this) }

  override string getAPrimaryQlClass() { result = "EnumType" }
}

/** An enum constant. */
class EnumConstant extends Field {
  EnumConstant() { isEnumConst(this) }

  override string getAPrimaryQlClass() { result = "EnumConstant" }
}

/** A record type. */
class Record extends Class {
  Record() { isRecord(this) }

  override string getAPrimaryQlClass() { result = "Record" }
}

/** A type variable (generic type parameter). */
class TypeVariable extends RefType, @typevariable {
  /** Gets the upper bound type of this type variable. */
  RefType getATypeBound() { typeBounds(_, result, _, this) }

  /** Holds if this type variable has a bound. */
  predicate hasTypeBound() { exists(this.getATypeBound()) }

  override string getAPrimaryQlClass() { result = "TypeVariable" }
}

/** A wildcard type. */
class Wildcard extends RefType, @wildcard {
  override string getAPrimaryQlClass() { result = "Wildcard" }
}

/** A type bound on a type variable or wildcard. */
class TypeBound extends @typebound {
  /** Gets the type of this bound. */
  RefType getType() { typeBounds(this, result, _, _) }

  /** Gets the position of this bound. */
  int getPosition() { typeBounds(this, _, result, _) }
}

/**
 * The type `java.lang.Object`.
 */
class TypeObject extends Class {
  TypeObject() {
    this.hasName("Object") and this.getPackage().hasName("java.lang")
  }
}

/**
 * The type `java.lang.String`.
 */
class TypeString extends Class {
  TypeString() {
    this.hasName("String") and this.getPackage().hasName("java.lang")
  }
}
