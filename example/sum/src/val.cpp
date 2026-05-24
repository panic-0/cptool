#include "testlib.h"

constexpr int N = 1e5;
constexpr int K = 1e5;
constexpr int V = 1e9;

int main(int argc, char *argv[]) {
    registerValidation(argc, argv);

    int n = inf.readInt(1, N, "n");
    inf.readSpace();
    inf.readInt(1, K, "k");
    inf.readEoln();
    for (int i = 0; i < n; ++i) {
        if (i) {
            inf.readSpace();
        }
        inf.readInt(0, V, "a_i");
    }
    inf.readEoln();
    inf.readEof();

    return 0;
}
