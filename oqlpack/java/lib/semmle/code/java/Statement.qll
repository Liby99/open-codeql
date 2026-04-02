/**
 * Provides classes and predicates for working with Java statements.
 */

import Expr

/** A statement. */
class Stmt extends StmtParent, ExprParent, @stmt {
  /** Gets the kind of this statement. */
  int getKind() { stmts(this, result, _, _, _) }

  /** Gets the parent of this statement. */
  StmtParent getParent() { stmts(this, _, result, _, _) }

  /** Gets the index of this statement in its parent. */
  int getIndex() { stmts(this, _, _, result, _) }

  /** Gets the enclosing callable. */
  Callable getEnclosingCallable() { stmts(this, _, _, _, result) }

  /** Gets the child statement at the given index. */
  Stmt getChild(int index) { stmts(result, _, this, index, _) }

  override string getAPrimaryQlClass() { result = "Stmt" }
}

/** A conditional statement (if, for, while, do). */
class ConditionalStmt extends Stmt {
  ConditionalStmt() {
    this instanceof IfStmt or this instanceof ForStmt or
    this instanceof WhileStmt or this instanceof DoStmt
  }

  /** Gets the condition expression. */
  Expr getCondition() { none() }
}

/** A loop statement. */
class LoopStmt extends Stmt {
  LoopStmt() {
    this instanceof ForStmt or this instanceof EnhancedForStmt or
    this instanceof WhileStmt or this instanceof DoStmt
  }

  /** Gets the body of this loop. */
  Stmt getBody() { none() }
}

/** A block statement. */
class BlockStmt extends Stmt {
  BlockStmt() { this.getKind() = 0 }

  /** Gets the statement at the given index. */
  Stmt getStmt(int i) { result = this.getChild(i) }

  /** Gets a statement in this block. */
  Stmt getAStmt() { result = this.getStmt(_) }

  /** Gets the number of statements in this block. */
  int getNumStmt() { result = count(int i | exists(this.getStmt(i))) }

  override string getAPrimaryQlClass() { result = "BlockStmt" }
}

/** An if statement. */
class IfStmt extends Stmt {
  IfStmt() { this.getKind() = 6 }

  /** Gets the condition. */
  Expr getCondition() { result.isNthChildOf(this, 0) }

  /** Gets the "then" branch. */
  Stmt getThen() { result = this.getChild(1) }

  /** Gets the "else" branch, if any. */
  Stmt getElse() { result = this.getChild(2) }

  override string getAPrimaryQlClass() { result = "IfStmt" }
}

/** A for statement. */
class ForStmt extends Stmt {
  ForStmt() { this.getKind() = 7 }

  override string getAPrimaryQlClass() { result = "ForStmt" }
}

/** An enhanced for (for-each) statement. */
class EnhancedForStmt extends Stmt {
  EnhancedForStmt() { this.getKind() = 8 }

  override string getAPrimaryQlClass() { result = "EnhancedForStmt" }
}

/** A while statement. */
class WhileStmt extends Stmt {
  WhileStmt() { this.getKind() = 9 }

  /** Gets the condition. */
  Expr getCondition() { result.isNthChildOf(this, 0) }

  /** Gets the body. */
  Stmt getBody() { result = this.getChild(1) }

  override string getAPrimaryQlClass() { result = "WhileStmt" }
}

/** A do-while statement. */
class DoStmt extends Stmt {
  DoStmt() { this.getKind() = 10 }

  override string getAPrimaryQlClass() { result = "DoStmt" }
}

/** A try statement. */
class TryStmt extends Stmt {
  TryStmt() { this.getKind() = 15 }

  /** Gets the block of the try statement. */
  BlockStmt getBlock() { result = this.getChild(0) }

  override string getAPrimaryQlClass() { result = "TryStmt" }
}

/** A return statement. */
class ReturnStmt extends Stmt {
  ReturnStmt() { this.getKind() = 18 }

  /** Gets the returned expression, if any. */
  Expr getResult() { result.isNthChildOf(this, 0) }

  override string getAPrimaryQlClass() { result = "ReturnStmt" }
}

/** A throw statement. */
class ThrowStmt extends Stmt {
  ThrowStmt() { this.getKind() = 19 }

  /** Gets the thrown expression. */
  Expr getExpr() { result.isNthChildOf(this, 0) }

  override string getAPrimaryQlClass() { result = "ThrowStmt" }
}

/** A switch statement. */
class SwitchStmt extends Stmt {
  SwitchStmt() { this.getKind() = 14 }

  override string getAPrimaryQlClass() { result = "SwitchStmt" }
}

/** An expression statement. */
class ExprStmt extends Stmt {
  ExprStmt() { this.getKind() = 1 }

  /** Gets the expression. */
  Expr getExpr() { result.isNthChildOf(this, 0) }

  override string getAPrimaryQlClass() { result = "ExprStmt" }
}

/** An empty statement. */
class EmptyStmt extends Stmt {
  EmptyStmt() { this.getKind() = 16 }

  override string getAPrimaryQlClass() { result = "EmptyStmt" }
}

/** A break statement. */
class BreakStmt extends Stmt {
  BreakStmt() { this.getKind() = 20 }

  override string getAPrimaryQlClass() { result = "BreakStmt" }
}

/** A continue statement. */
class ContinueStmt extends Stmt {
  ContinueStmt() { this.getKind() = 21 }

  override string getAPrimaryQlClass() { result = "ContinueStmt" }
}

/** A local variable declaration statement. */
class LocalVariableDeclStmt extends Stmt {
  LocalVariableDeclStmt() { this.getKind() = 11 }

  override string getAPrimaryQlClass() { result = "LocalVariableDeclStmt" }
}

/** An assert statement. */
class AssertStmt extends Stmt {
  AssertStmt() { this.getKind() = 22 }

  override string getAPrimaryQlClass() { result = "AssertStmt" }
}

/** A synchronized statement. */
class SynchronizedStmt extends Stmt {
  SynchronizedStmt() { this.getKind() = 17 }

  override string getAPrimaryQlClass() { result = "SynchronizedStmt" }
}

/** A super constructor invocation statement. */
class SuperConstructorInvocationStmt extends Stmt {
  SuperConstructorInvocationStmt() { this.getKind() = 3 }

  override string getAPrimaryQlClass() { result = "SuperConstructorInvocationStmt" }
}

/** A this constructor invocation statement. */
class ThisConstructorInvocationStmt extends Stmt {
  ThisConstructorInvocationStmt() { this.getKind() = 4 }

  override string getAPrimaryQlClass() { result = "ThisConstructorInvocationStmt" }
}

/** A super method call. */
class SuperMethodCall extends MethodCall {
  SuperMethodCall() { none() }

  override string getAPrimaryQlClass() { result = "SuperMethodCall" }
}
