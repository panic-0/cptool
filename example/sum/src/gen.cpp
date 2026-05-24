#include "testlib.h"
#include <vector>

void generate(int argc, char *argv[]) {
    (void)argc;
    (void)argv;

    const int max_n = opt<int>(1);
    const int max_k = opt<int>(2);
    const int max_a = opt<int>(3);
    const int min_k = opt<int>(4);

    const int n = rnd.next(1, max_n);
    const int k = rnd.next(min_k, max_k);

    println(n, k);

    std::vector<int> a;
    for (int i = 0; i < n; ++i) {
        a.push_back(rnd.next(0, max_a));
    }
    println(a.begin(), a.end());
}

int main(int argc, char *argv[]) {
    registerGen(argc, argv, 1);

    generate(argc, argv);

    return 0;
}
