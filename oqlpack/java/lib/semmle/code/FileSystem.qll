/** Provides classes for working with files and folders. */

import Location

/** A file or folder. */
class Container extends @container, Top {
  /** Gets the absolute path of this container. */
  string getAbsolutePath() { none() }

  /** Gets the parent container. */
  Container getParentContainer() { containerparent(result, this) }

  /** Gets the base name of this container. */
  string getBaseName() {
    result = this.getAbsolutePath().regexpCapture(".*/(.*)", 1)
  }

  override string toString() { result = this.getAbsolutePath() }
}

/** A folder. */
class Folder extends Container, @folder {
  override string getAbsolutePath() { folders(this, result) }

  override string getAPrimaryQlClass() { result = "Folder" }
}

/** A file. */
class File extends Container, @file {
  override string getAbsolutePath() { files(this, result) }

  /** Gets the extension of this file. */
  string getExtension() {
    result = this.getAbsolutePath().regexpCapture(".*\\.(.*)", 1)
  }

  /** Gets the stem (name without extension) of this file. */
  string getStem() {
    result = this.getBaseName().regexpCapture("(.*)\\..*", 1)
    or
    not this.getBaseName().matches("%.%") and result = this.getBaseName()
  }

  /** Holds if this is a Java source file. */
  predicate isSourceFile() { this.isJavaSourceFile() }

  /** Holds if this is a Java source file. */
  predicate isJavaSourceFile() { this.getExtension() = "java" }

  override string getAPrimaryQlClass() { result = "File" }
}
