/*
** Test: Realistic C code in the style of SQLite.
** Tests complex struct hierarchies, callbacks, error handling patterns.
*/

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* Error codes */
#define DB_OK          0
#define DB_ERROR      -1
#define DB_NOMEM      -2
#define DB_NOTFOUND   -3

/* Forward declarations */
typedef struct Database Database;
typedef struct Statement Statement;
typedef struct Row Row;
typedef struct Column Column;

/* Callback for query results */
typedef int (*query_callback)(void* user_data, int ncols, char** values, char** names);

/* Column types */
enum ColumnType {
    COL_INTEGER = 1,
    COL_TEXT    = 2,
    COL_REAL   = 3,
    COL_BLOB   = 4,
    COL_NULL   = 5
};

/* Column definition */
struct Column {
    char name[64];
    enum ColumnType type;
    int not_null;
    int primary_key;
};

/* Table definition */
struct Table {
    char name[64];
    int ncols;
    struct Column* columns;
    int nrows;
    int capacity;
    char*** rows;  /* array of string arrays */
};

/* Database handle */
struct Database {
    char* path;
    int ntables;
    struct Table* tables;
    char last_error[256];
    int is_open;
};

/* Create a new database */
Database* db_open(const char* path) {
    Database* db = calloc(1, sizeof(Database));
    if (!db) return NULL;
    db->path = strdup(path);
    db->is_open = 1;
    return db;
}

/* Close database */
void db_close(Database* db) {
    if (!db) return;
    for (int i = 0; i < db->ntables; i++) {
        struct Table* t = &db->tables[i];
        for (int r = 0; r < t->nrows; r++) {
            for (int c = 0; c < t->ncols; c++) {
                free(t->rows[r][c]);
            }
            free(t->rows[r]);
        }
        free(t->rows);
        free(t->columns);
    }
    free(db->tables);
    free(db->path);
    free(db);
}

/* Get last error message */
const char* db_errmsg(Database* db) {
    return db->last_error;
}

/* Create a table */
int db_create_table(Database* db, const char* name,
                    const Column* cols, int ncols) {
    if (!db || !db->is_open) return DB_ERROR;

    /* Check for duplicate */
    for (int i = 0; i < db->ntables; i++) {
        if (strcmp(db->tables[i].name, name) == 0) {
            snprintf(db->last_error, sizeof(db->last_error),
                     "table '%s' already exists", name);
            return DB_ERROR;
        }
    }

    /* Grow tables array */
    int new_count = db->ntables + 1;
    struct Table* new_tables = realloc(db->tables,
                                       new_count * sizeof(struct Table));
    if (!new_tables) return DB_NOMEM;
    db->tables = new_tables;

    struct Table* t = &db->tables[db->ntables];
    memset(t, 0, sizeof(struct Table));
    strncpy(t->name, name, sizeof(t->name) - 1);
    t->ncols = ncols;
    t->columns = malloc(ncols * sizeof(Column));
    if (!t->columns) return DB_NOMEM;
    memcpy(t->columns, cols, ncols * sizeof(Column));
    t->capacity = 16;
    t->rows = calloc(t->capacity, sizeof(char**));
    if (!t->rows) { free(t->columns); return DB_NOMEM; }

    db->ntables = new_count;
    return DB_OK;
}

/* Insert a row */
int db_insert(Database* db, const char* table_name,
              const char** values, int nvalues) {
    if (!db || !db->is_open) return DB_ERROR;

    struct Table* t = NULL;
    for (int i = 0; i < db->ntables; i++) {
        if (strcmp(db->tables[i].name, table_name) == 0) {
            t = &db->tables[i];
            break;
        }
    }
    if (!t) {
        snprintf(db->last_error, sizeof(db->last_error),
                 "no such table: %s", table_name);
        return DB_NOTFOUND;
    }

    if (nvalues != t->ncols) return DB_ERROR;

    /* Grow if needed */
    if (t->nrows >= t->capacity) {
        int new_cap = t->capacity * 2;
        char*** new_rows = realloc(t->rows, new_cap * sizeof(char**));
        if (!new_rows) return DB_NOMEM;
        t->rows = new_rows;
        t->capacity = new_cap;
    }

    /* Copy values */
    char** row = malloc(nvalues * sizeof(char*));
    if (!row) return DB_NOMEM;
    for (int i = 0; i < nvalues; i++) {
        row[i] = strdup(values[i] ? values[i] : "");
    }
    t->rows[t->nrows++] = row;

    return DB_OK;
}

/* Execute a query with callback (like sqlite3_exec) */
int db_exec(Database* db, const char* table_name,
            query_callback cb, void* user_data) {
    if (!db || !db->is_open) return DB_ERROR;

    struct Table* t = NULL;
    for (int i = 0; i < db->ntables; i++) {
        if (strcmp(db->tables[i].name, table_name) == 0) {
            t = &db->tables[i];
            break;
        }
    }
    if (!t) return DB_NOTFOUND;

    /* Build column name array */
    char* names[64];
    for (int c = 0; c < t->ncols; c++) {
        names[c] = t->columns[c].name;
    }

    /* Iterate rows */
    for (int r = 0; r < t->nrows; r++) {
        int rc = cb(user_data, t->ncols, t->rows[r], names);
        if (rc != 0) return rc;
    }

    return DB_OK;
}

/* Helper: print callback */
static int print_row(void* data, int ncols, char** values, char** names) {
    for (int i = 0; i < ncols; i++) {
        printf("%s = %s%s", names[i], values[i],
               (i < ncols - 1) ? ", " : "\n");
    }
    return 0;
}

int main(void) {
    Database* db = db_open(":memory:");
    if (!db) {
        fprintf(stderr, "Failed to open database\n");
        return 1;
    }

    Column cols[] = {
        {"id",   COL_INTEGER, 1, 1},
        {"name", COL_TEXT,    1, 0},
        {"age",  COL_INTEGER, 0, 0}
    };
    db_create_table(db, "users", cols, 3);

    const char* row1[] = {"1", "Alice", "30"};
    const char* row2[] = {"2", "Bob", "25"};
    const char* row3[] = {"3", "Charlie", "35"};
    db_insert(db, "users", row1, 3);
    db_insert(db, "users", row2, 3);
    db_insert(db, "users", row3, 3);

    printf("All users:\n");
    db_exec(db, "users", print_row, NULL);

    db_close(db);
    return 0;
}
