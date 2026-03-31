/** A function in the program. */
class Function extends int {
    Function() { functions(this, _, _) }

    /** Gets the name of this function. */
    string getName() { functions(this, result, _) }

    /** Gets the kind of this function (1=definition, 2=declaration). */
    int getKind() { functions(this, _, result) }

    /** Holds if this function has at least one parameter. */
    predicate hasParameters() { params(this, _, _, _) }

    /** Gets a parameter name of this function. */
    string getAParameterName() {
        params(this, _, result, _)
    }
}
