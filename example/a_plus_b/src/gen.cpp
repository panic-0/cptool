#include <iostream>
#include <random>

int main(int argc, char *argv[]) {
    const int V = std::atoi(argv[1]);

    std::mt19937 rnd;
    std::uniform_int_distribution<int> dist(1, V);
    std::cout << dist(rnd) << " " << dist(rnd) << "\n";

    return 0;
}
