/* Test: C data structures — linked list, hash table, function pointers */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* Linked list */
struct Node {
    int data;
    struct Node* next;
};

struct LinkedList {
    struct Node* head;
    int size;
};

struct LinkedList* list_create(void) {
    struct LinkedList* list = malloc(sizeof(struct LinkedList));
    list->head = NULL;
    list->size = 0;
    return list;
}

void list_push(struct LinkedList* list, int value) {
    struct Node* node = malloc(sizeof(struct Node));
    node->data = value;
    node->next = list->head;
    list->head = node;
    list->size++;
}

int list_pop(struct LinkedList* list) {
    if (list->head == NULL) return -1;
    struct Node* node = list->head;
    int value = node->data;
    list->head = node->next;
    free(node);
    list->size--;
    return value;
}

void list_destroy(struct LinkedList* list) {
    struct Node* current = list->head;
    while (current != NULL) {
        struct Node* next = current->next;
        free(current);
        current = next;
    }
    free(list);
}

/* Simple hash table */
#define TABLE_SIZE 256

typedef unsigned int (*hash_func_t)(const char*);

struct HashEntry {
    char* key;
    int value;
    struct HashEntry* next;
};

struct HashTable {
    struct HashEntry* buckets[TABLE_SIZE];
    hash_func_t hash;
};

unsigned int djb2_hash(const char* str) {
    unsigned int hash = 5381;
    int c;
    while ((c = *str++))
        hash = ((hash << 5) + hash) + c;
    return hash % TABLE_SIZE;
}

struct HashTable* table_create(hash_func_t hash_fn) {
    struct HashTable* table = calloc(1, sizeof(struct HashTable));
    table->hash = hash_fn;
    return table;
}

void table_put(struct HashTable* table, const char* key, int value) {
    unsigned int idx = table->hash(key);
    struct HashEntry* entry = malloc(sizeof(struct HashEntry));
    entry->key = strdup(key);
    entry->value = value;
    entry->next = table->buckets[idx];
    table->buckets[idx] = entry;
}

int table_get(struct HashTable* table, const char* key, int default_val) {
    unsigned int idx = table->hash(key);
    struct HashEntry* entry = table->buckets[idx];
    while (entry != NULL) {
        if (strcmp(entry->key, key) == 0)
            return entry->value;
        entry = entry->next;
    }
    return default_val;
}

void table_destroy(struct HashTable* table) {
    for (int i = 0; i < TABLE_SIZE; i++) {
        struct HashEntry* entry = table->buckets[i];
        while (entry != NULL) {
            struct HashEntry* next = entry->next;
            free(entry->key);
            free(entry);
            entry = next;
        }
    }
    free(table);
}

/* Function pointer array (dispatch table) */
typedef int (*binary_op)(int, int);

int op_add(int a, int b) { return a + b; }
int op_sub(int a, int b) { return a - b; }
int op_mul(int a, int b) { return a * b; }
int op_div(int a, int b) { return b != 0 ? a / b : 0; }

static binary_op operations[] = {op_add, op_sub, op_mul, op_div};
static const char* op_names[] = {"add", "sub", "mul", "div"};

int main(void) {
    /* Linked list test */
    struct LinkedList* list = list_create();
    list_push(list, 10);
    list_push(list, 20);
    list_push(list, 30);
    int val = list_pop(list);
    printf("Popped: %d, size: %d\n", val, list->size);
    list_destroy(list);

    /* Hash table test */
    struct HashTable* table = table_create(djb2_hash);
    table_put(table, "alice", 30);
    table_put(table, "bob", 25);
    printf("alice = %d\n", table_get(table, "alice", -1));
    table_destroy(table);

    /* Dispatch table test */
    for (int i = 0; i < 4; i++) {
        printf("%s(10, 3) = %d\n", op_names[i], operations[i](10, 3));
    }

    return 0;
}
