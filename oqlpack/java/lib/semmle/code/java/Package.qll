/**
 * Provides classes and predicates for working with Java packages.
 */

import Element

/** A Java package. */
class Package extends Element, @package {
  override string getAPrimaryQlClass() { result = "Package" }

  /** Holds if this package has the specified `name`. */
  override predicate hasName(string name) { packages(this, name) }

  /** Gets a type in this package. */
  RefType getAType() { classes_or_interfaces(result, _, this, _) }

  /** Gets a compilation unit in this package. */
  CompilationUnit getACompilationUnit() { cupackage(result, this) }
}
