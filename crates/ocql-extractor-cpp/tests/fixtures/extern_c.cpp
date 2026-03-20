// Test: extern "C" blocks, C/C++ interop patterns, static functions

#include <cstring>
#include <cstdlib>

// C-compatible API wrapped in extern "C"
extern "C" {

struct CBuffer {
    char* data;
    int size;
    int capacity;
};

CBuffer* cbuffer_create(int capacity) {
    CBuffer* buf = (CBuffer*)malloc(sizeof(CBuffer));
    buf->data = (char*)malloc(capacity);
    buf->size = 0;
    buf->capacity = capacity;
    return buf;
}

void cbuffer_destroy(CBuffer* buf) {
    if (buf) {
        free(buf->data);
        free(buf);
    }
}

int cbuffer_append(CBuffer* buf, const char* data, int len) {
    if (buf->size + len > buf->capacity) return -1;
    memcpy(buf->data + buf->size, data, len);
    buf->size += len;
    return 0;
}

} // extern "C"

// Static helper (internal linkage)
static int validate_input(const char* input) {
    if (input == nullptr) return 0;
    if (strlen(input) == 0) return 0;
    return 1;
}

// C++ wrapper class around C API
class BufferWrapper {
public:
    BufferWrapper(int capacity) : buf_(cbuffer_create(capacity)) {}
    ~BufferWrapper() { cbuffer_destroy(buf_); }

    bool append(const char* data) {
        if (!validate_input(data)) return false;
        int len = strlen(data);
        return cbuffer_append(buf_, data, len) == 0;
    }

    int size() const { return buf_->size; }
    const char* data() const { return buf_->data; }

private:
    CBuffer* buf_;
};

int main() {
    BufferWrapper buf(1024);
    buf.append("hello ");
    buf.append("world");
    return 0;
}
