/**
 * Provides classes and predicates for working with Java compilation units.
 */

import Element
import Package
import semmle.code.FileSystem

/** A compilation unit is a source file. */
class CompilationUnit extends File {
  /** Gets the package of this compilation unit. */
  Package getPackage() { cupackage(this, result) }
}
