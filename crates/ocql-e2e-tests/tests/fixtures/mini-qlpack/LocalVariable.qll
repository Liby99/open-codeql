/** A local variable. */
class LocalVariable extends int {
    LocalVariable() { localvariables(this, _, _) }

    /** Gets the name of this variable. */
    string getName() { localvariables(this, result, _) }

    /** Gets the type of this variable. */
    string getType() { localvariables(this, _, result) }
}
