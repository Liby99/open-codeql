/**
 * Provides classes and predicates for working with members of Java classes and interfaces,
 * that is, methods, constructors, fields and nested types.
 */

import Element
import Type
import Modifier

/**
 * A common abstraction for type member declarations,
 * including methods, constructors, fields, and nested types.
 */
class Member extends Element, Annotatable, Modifiable, @member {
  Member() { declaresMember(_, this) }

  /** Gets the type in which this member is declared. */
  RefType getDeclaringType() { declaresMember(result, this) }

  /**
   * Gets the qualified name of this member.
   */
  string getQualifiedName() {
    result = this.getDeclaringType().getQualifiedName() + "." + this.getName()
  }

  /**
   * Holds if this member has the specified name and is declared in the
   * specified package and type.
   */
  predicate hasQualifiedName(string package, string type, string name) {
    this.getDeclaringType().hasQualifiedName(package, type) and this.hasName(name)
  }

  /** Holds if this member is package protected. */
  predicate isPackageProtected() {
    not this.isPrivate() and
    not this.isProtected() and
    not this.isPublic()
  }
}

/** A parent of a statement. */
class StmtParent extends @stmtparent, Top {
}

/** A parent of an expression. */
class ExprParent extends @exprparent, Top {
}

/** A callable is a method or constructor. */
class Callable extends StmtParent, Member, @callable {
  /**
   * Gets the declared return type of this callable.
   */
  Type getReturnType() {
    constrs(this, _, _, result, _, _) or
    methods(this, _, _, result, _, _)
  }

  /** Holds if this callable calls `target`. */
  predicate calls(Callable target) {
    exists(Call call |
      call.getEnclosingCallable() = this and
      call.getCallee() = target
    )
  }

  /** Gets the number of formal parameters of this callable. */
  int getNumberOfParameters() { result = count(this.getAParameter()) }

  /** Gets a formal parameter of this callable. */
  Parameter getAParameter() { result.getCallable() = this }

  /** Gets the formal parameter at the specified (zero-based) position. */
  Parameter getParameter(int n) { params(result, _, n, this, _) }

  /** Gets the type of the formal parameter at the specified (zero-based) position. */
  Type getParameterType(int n) { params(_, result, n, this, _) }

  /** Holds if this callable has no parameters. */
  predicate hasNoParameters() { not exists(this.getAParameter()) }

  /** Gets a type of a parameter. */
  Type getAParamType() { result = this.getParameterType(_) }

  /**
   * Gets the string signature of this callable.
   */
  string getStringSignature() { result = this.getName() + this.paramsString() }

  /**
   * Gets a parenthesized string of parameter types.
   */
  string paramsString() {
    exists(int n | n = this.getNumberOfParameters() |
      n = 0 and result = "()"
      or
      n > 0 and result = "(" + this.paramUpTo(n - 1) + ")"
    )
  }

  private string paramUpTo(int n) {
    n = 0 and result = this.getParameterType(0).toString()
    or
    n > 0 and result = this.paramUpTo(n - 1) + ", " + this.getParameterType(n)
  }

  /** Holds if this callable has the specified string signature. */
  predicate hasStringSignature(string sig) { sig = this.getStringSignature() }

  /**
   * Gets the signature of this callable, as it appears in the bytecode.
   */
  string getSignature() {
    methods(this, _, result, _, _, _) or constrs(this, _, result, _, _, _)
  }

  /** Gets the source declaration of this callable. */
  Callable getSourceDeclaration() { result = this }

  /** Holds if this callable is the same as its source declaration. */
  predicate isSourceDeclaration() { this.getSourceDeclaration() = this }

  /** Holds if the last parameter of this callable is a varargs parameter. */
  predicate isVarargs() { this.getAParameter().isVarargs() }

  /** Gets the body of this callable, if any. */
  BlockStmt getBody() { result.getParent() = this }

  /** Gets a call site that references this callable. */
  Call getAReference() { result.getCallee() = this }

  /** Holds if this callable is inheritable by subtypes of the declaring type. */
  predicate isInheritable() {
    not this.isPrivate() and not this instanceof Constructor
  }

  override string getAPrimaryQlClass() { result = "Callable" }
}

/** A method. */
class Method extends Callable, @method {
  /** Gets the source declaration of this method. */
  override Method getSourceDeclaration() { methods(this, _, _, _, _, result) }

  /**
   * Holds if this method is abstract, either explicitly or implicitly.
   * JLS 9.4: An interface method lacking a `private`, `default`, or `static`
   * modifier is implicitly abstract.
   */
  override predicate isAbstract() {
    this.hasModifier("abstract")
    or
    this.getDeclaringType() instanceof Interface and
    not this.isPrivate() and
    not this.isDefault() and
    not this.isStatic()
  }

  /** Holds if this method overrides `m`. */
  predicate overrides(Method m) {
    exists(RefType t | t = this.getDeclaringType() |
      t.extendsOrImplements+(m.getDeclaringType()) and
      this.getName() = m.getName() and
      this.getSignature() = m.getSignature() and
      this != m
    )
  }

  /** Gets a method overridden by this method. */
  Method getAnOverride() { this.overrides(result) }

  override string getAPrimaryQlClass() { result = "Method" }
}

/** A constructor. */
class Constructor extends Callable, @constructor {
  /** Gets the source declaration of this constructor. */
  override Constructor getSourceDeclaration() { constrs(this, _, _, _, _, result) }

  /** Holds if this is a default constructor. */
  predicate isDefaultConstructor() { isDefConstr(this) }

  override string getAPrimaryQlClass() { result = "Constructor" }
}

/** A field. */
class Field extends Member, ExprParent, @field, Variable {
  /** Gets the type of this field. */
  override Type getType() { fields(this, _, result, _) }

  /** Gets the declaring type of this field. */
  override RefType getDeclaringType() { fields(this, _, _, result) }

  /** Gets the initializer of this field, if any. */
  override Expr getInitializer() { none() }

  override string getAPrimaryQlClass() { result = "Field" }
}
