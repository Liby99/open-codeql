/**
 * Provides classes and predicates for working with Java expressions.
 *
 * Expression kind values match the vendor dbscheme case @expr.kind:
 *   1=arrayaccess, 2=arraycreation, 3=arrayinit, 4=assign, 5-15=compound assign,
 *   16=boolean_lit, 17=int_lit, 18=long_lit, 19=float_lit, 20=double_lit,
 *   21=char_lit, 22=string_lit, 23=null_lit, 24-28=arith, 29-31=shift,
 *   32-34=bitwise, 35-36=logical, 37-42=comparison, 43-50=unary,
 *   51=cast, 52=new, 53=conditional, 55=instanceof, 56=localvardecl,
 *   58=this, 59=super, 60=varaccess, 61=methodaccess, 62=typeaccess, 68=lambda
 */

import Element
import Type
import Member
import Variable

/** An expression. */
class Expr extends ExprParent, @expr {
  /** Gets the kind of this expression. */
  int getKind() { exprs(this, result, _, _, _) }

  /** Gets the type of this expression. */
  Type getType() { exprs(this, _, result, _, _) }

  /** Gets the parent of this expression. */
  ExprParent getParent() { exprs(this, _, _, result, _) }

  /** Gets the index of this expression within its parent. */
  int getIndex() { exprs(this, _, _, _, result) }

  /** Gets a child expression at the given index. */
  Expr getChildExpr(int index) { exprs(result, _, _, this, index) }

  /** Gets the number of child expressions. */
  int getNumChildExpr() { result = count(int i | exists(this.getChildExpr(i))) }

  /** Gets the enclosing callable. */
  Callable getEnclosingCallable() { callableEnclosingExpr(this, result) }

  /**
   * Holds if this expression is the child of `parent` at the given `index`.
   */
  predicate isNthChildOf(ExprParent parent, int index) { exprs(this, _, _, parent, index) }

  override string getAPrimaryQlClass() { result = "Expr" }
}

/** A call to a method or constructor. */
class Call extends Expr {
  Call() { callableBinding(this, _) }

  /** Gets the callee (method or constructor) called. */
  Callable getCallee() { callableBinding(this, result) }

  /** Gets the caller of this call. */
  Callable getCaller() { this.getEnclosingCallable() = result }

  /** Gets the argument at the given (zero-based) position. */
  Expr getArgument(int n) { result = this.getChildExpr(n) }

  /** Gets an argument to this call. */
  Expr getAnArgument() { result = this.getArgument(_) }

  /** Gets the number of arguments. */
  int getNumArgument() { result = count(int i | exists(this.getArgument(i))) }

  override string getAPrimaryQlClass() { result = "Call" }
}

/** A method call expression. */
class MethodCall extends Call {
  MethodCall() { this.getCallee() instanceof Method }

  /** Gets the method being called. */
  Method getMethod() { result = this.getCallee() }

  /** Gets the qualifier expression, if any. */
  Expr getQualifier() { none() }

  override string getAPrimaryQlClass() { result = "MethodCall" }
}

/** A constructor call (class instance creation) expression. */
class ClassInstanceExpr extends Call {
  ClassInstanceExpr() { this.getCallee() instanceof Constructor }

  override string getAPrimaryQlClass() { result = "ClassInstanceExpr" }
}

/** A variable access expression. */
class VarAccess extends Expr {
  VarAccess() { variableBinding(this, _) }

  /** Gets the variable being accessed. */
  Variable getVariable() { variableBinding(this, result) }

  override string getAPrimaryQlClass() { result = "VarAccess" }
}

/** A variable write (assignment target). */
class VarWrite extends VarAccess {
  VarWrite() { exists(AssignExpr a | a.getDest() = this) }
}

/** A variable read (non-assignment access). */
class VarRead extends VarAccess {
  VarRead() { not exists(AssignExpr a | a.getDest() = this) }
}

/** A field access expression. */
class FieldAccess extends VarAccess {
  FieldAccess() { this.getVariable() instanceof Field }

  /** Gets the field being accessed. */
  Field getField() { result = this.getVariable() }

  /** Gets the qualifier expression, if any. */
  Expr getQualifier() { none() }

  override string getAPrimaryQlClass() { result = "FieldAccess" }
}

/** A field read. */
class FieldRead extends FieldAccess, VarRead { }

/** A field write. */
class FieldWrite extends FieldAccess, VarWrite { }

/** An assignment expression. */
class AssignExpr extends Expr {
  AssignExpr() { this.getKind() = 4 }

  /** Gets the destination of the assignment (left-hand side). */
  Expr getDest() { result = this.getChildExpr(0) }

  /** Gets the source of the assignment (right-hand side). */
  Expr getSource() { result = this.getChildExpr(1) }

  override string getAPrimaryQlClass() { result = "AssignExpr" }
}

/** A literal expression. */
class Literal extends Expr {
  Literal() {
    this.getKind() >= 16 and this.getKind() <= 23
  }

  /** Gets the literal value as a string. */
  string getValue() { namestrings(_, result, this) }

  override string getAPrimaryQlClass() { result = "Literal" }
}

/** A string literal. */
class StringLiteral extends Literal {
  StringLiteral() { this.getKind() = 22 }

  override string getAPrimaryQlClass() { result = "StringLiteral" }
}

/** An integer literal. */
class IntegerLiteral extends Literal {
  IntegerLiteral() { this.getKind() = 17 }

  override string getAPrimaryQlClass() { result = "IntegerLiteral" }
}

/** A `this` expression. */
class ThisAccess extends Expr {
  ThisAccess() { this.getKind() = 58 }

  override string getAPrimaryQlClass() { result = "ThisAccess" }
}

/** A `super` expression. */
class SuperAccess extends Expr {
  SuperAccess() { this.getKind() = 59 }

  override string getAPrimaryQlClass() { result = "SuperAccess" }
}

/** A type access expression. */
class TypeAccess extends Expr {
  TypeAccess() { this.getKind() = 62 }

  override string getAPrimaryQlClass() { result = "TypeAccess" }
}

/** A local variable declaration expression. */
class LocalVariableDeclExpr extends Expr {
  LocalVariableDeclExpr() { this.getKind() = 56 }

  /** Gets the local variable. */
  LocalVariableDecl getVariable() { localvars(result, _, _, this) }

  /** Gets the initializer, if any. */
  Expr getInit() { result = this.getChildExpr(0) }

  /** Gets the init or pattern source. */
  Expr getInitOrPatternSource() { result = this.getInit() }

  override string getAPrimaryQlClass() { result = "LocalVariableDeclExpr" }
}

/** A cast expression. */
class CastExpr extends Expr {
  CastExpr() { this.getKind() = 51 }

  /** Gets the expression being cast. */
  Expr getExpr() { result = this.getChildExpr(0) }

  /** Gets the target type of the cast. */
  Expr getTypeExpr() { result = this.getChildExpr(1) }

  override string getAPrimaryQlClass() { result = "CastExpr" }
}

/** An `instanceof` expression. */
class InstanceOfExpr extends Expr {
  InstanceOfExpr() { this.getKind() = 55 }

  override string getAPrimaryQlClass() { result = "InstanceOfExpr" }
}

/** A conditional expression (`? :`). */
class ConditionalExpr extends Expr {
  ConditionalExpr() { this.getKind() = 53 }

  /** Gets the condition. */
  Expr getCondition() { result = this.getChildExpr(0) }

  /** Gets the "then" expression. */
  Expr getTrueExpr() { result = this.getChildExpr(1) }

  /** Gets the "else" expression. */
  Expr getFalseExpr() { result = this.getChildExpr(2) }

  override string getAPrimaryQlClass() { result = "ConditionalExpr" }
}
