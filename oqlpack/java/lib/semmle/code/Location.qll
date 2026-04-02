/** Provides classes and predicates for working with locations. */

import FileSystem
import semmle.code.java.Element

/** Holds if element `e` has name `name`. */
predicate hasName(Element e, string name) {
  classes_or_interfaces(e, name, _, _)
  or
  primitives(e, name)
  or
  constrs(e, name, _, _, _, _)
  or
  methods(e, name, _, _, _, _)
  or
  fields(e, name, _, _)
  or
  packages(e, name)
  or
  name = e.(File).getStem()
  or
  paramName(e, name)
  or
  exists(int pos |
    params(e, _, pos, _, _) and
    not paramName(e, _) and
    name = "p" + pos
  )
  or
  localvars(e, name, _, _)
  or
  typeVars(e, name, _, _)
  or
  wildcards(e, name, _)
  or
  arrays(e, name, _, _, _)
  or
  modifiers(e, name)
}

/**
 * Top is the root of the QL type hierarchy; it defines some default
 * methods for obtaining locations and a standard `toString()` method.
 */
class Top extends @top {
  /** Gets the source location for this element. */
  Location getLocation() { hasLocation(this, result) }

  /**
   * Holds if this element is at the specified location.
   * The location spans column `startcolumn` of line `startline` to
   * column `endcolumn` of line `endline` in file `filepath`.
   */
  predicate hasLocationInfo(
    string filepath, int startline, int startcolumn, int endline, int endcolumn
  ) {
    exists(File f, Location l | hasLocation(this, l) and f = l.getFile() |
      locations_default(l, f, startline, startcolumn, endline, endcolumn) and
      filepath = f.getAbsolutePath()
    )
  }

  /** Gets the file associated with this element. */
  File getFile() { hasLocation(this, result.getLocation()) }

  /** Gets the total number of lines that this element ranges over. */
  int getTotalNumberOfLines() { numlines(this, result, _, _) }

  /** Gets the number of lines of code that this element ranges over. */
  int getNumberOfLinesOfCode() { numlines(this, _, result, _) }

  /** Gets the number of comment lines that this element ranges over. */
  int getNumberOfCommentLines() { numlines(this, _, _, result) }

  /** Gets a textual representation of this element. */
  string toString() { hasName(this, result) }

  /**
   * Gets the name of a primary CodeQL class to which this element belongs.
   */
  string getAPrimaryQlClass() { result = "???" }
}

/** A location maps language elements to positions in source files. */
class Location extends @location {
  /** Gets the 1-based line number (inclusive) where this location starts. */
  int getStartLine() { locations_default(this, _, result, _, _, _) }

  /** Gets the 1-based column number (inclusive) where this location starts. */
  int getStartColumn() { locations_default(this, _, _, result, _, _) }

  /** Gets the 1-based line number (inclusive) where this location ends. */
  int getEndLine() { locations_default(this, _, _, _, result, _) }

  /** Gets the 1-based column number (inclusive) where this location ends. */
  int getEndColumn() { locations_default(this, _, _, _, _, result) }

  /** Gets the file containing this location. */
  File getFile() { locations_default(this, result, _, _, _, _) }

  /** Gets a string representation of this location. */
  string toString() {
    exists(string filepath, int startLine, int startCol, int endLine, int endCol |
      this.hasLocationInfo(filepath, startLine, startCol, endLine, endCol) and
      result = filepath + ":" + startLine + ":" + startCol + ":" + endLine + ":" + endCol
    )
  }

  /**
   * Holds if this element is at the specified location.
   */
  predicate hasLocationInfo(
    string filepath, int startline, int startcolumn, int endline, int endcolumn
  ) {
    exists(File f | locations_default(this, f, startline, startcolumn, endline, endcolumn) |
      filepath = f.getAbsolutePath()
    )
  }
}
