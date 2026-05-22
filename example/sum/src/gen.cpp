#include "testlib.h"
#include <vector>

void generate(int argc, char *argv[]) {
    const int n = opt<int>(1);
    const int v = opt<int>(2);

    println(n);

    std::vector<int> a;
    for (int i = 0; i < n; ++i) {
        a.push_back(rnd.next(0, v));
    }
    println(a.begin(), a.end());
}

int main(int argc, char *argv[]) {
    registerGen(argc, argv, 1);

    generate(argc, argv);

    return 0;
}
