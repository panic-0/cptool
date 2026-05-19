#include "testlib.h"

constexpr int V = 1e9;

int main(int argc, char *argv[]) {
    registerValidation(argc, argv);

    inf.readInt(1, V, "a");
    inf.readSpace();
    inf.readInt(1, V, "b");
    inf.readEoln();
    inf.readEof();

    return 0;
}
