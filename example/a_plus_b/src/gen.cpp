#include "testlib.h"

int main(int argc, char *argv[]) {
    registerGen(argc, argv, 1);

    const int v = opt<int>(1);
    println(rnd.next(1, v), rnd.next(1, v));

    return 0;
}
