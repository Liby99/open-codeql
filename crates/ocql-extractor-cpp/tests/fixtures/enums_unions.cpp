// Test: enums (C-style, scoped), unions, bitfields

#include <cstdint>
#include <string>

// C-style enum
enum Color {
    RED = 0,
    GREEN = 1,
    BLUE = 2
};

// Scoped enum (enum class)
enum class Direction : uint8_t {
    North = 0,
    South = 1,
    East = 2,
    West = 3
};

// Flags enum
enum Permissions {
    PERM_READ = 1,
    PERM_WRITE = 2,
    PERM_EXEC = 4
};

// Union
union Value {
    int i;
    float f;
    double d;
    char c;
};

// Tagged union (discriminated)
struct TaggedValue {
    enum Tag { INT, FLOAT, STRING } tag;
    union {
        int i;
        float f;
        // Can't put std::string in a union directly
    } data;
};

// Struct with bitfields
struct PackedFlags {
    unsigned int readable : 1;
    unsigned int writable : 1;
    unsigned int executable : 1;
    unsigned int reserved : 5;
    unsigned int type_id : 8;
};

// Enum used in function signatures
std::string direction_name(Direction dir) {
    switch (dir) {
        case Direction::North: return "North";
        case Direction::South: return "South";
        case Direction::East: return "East";
        case Direction::West: return "West";
    }
    return "Unknown";
}

Color blend(Color a, Color b) {
    return static_cast<Color>((static_cast<int>(a) + static_cast<int>(b)) / 2);
}

int main() {
    Color c = RED;
    Direction d = Direction::North;
    Value v;
    v.i = 42;
    v.f = 3.14f;

    PackedFlags flags = {1, 1, 0, 0, 5};

    TaggedValue tv;
    tv.tag = TaggedValue::INT;
    tv.data.i = 100;

    return 0;
}
