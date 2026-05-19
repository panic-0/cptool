#include <iostream>
#include <random>
#include <string>

std::mt19937 rnd;

void generate(int argc, char *argv[]) {
    const int N = atoi(argv[1]);
    const int V = atoi(argv[2]);

    std::cout << N << "\n";

    std::uniform_int_distribution<int> dist(0, V);
    for (int i = 0; i < N; ++i) {
        std::cout << dist(rnd) << " \n"[i == N - 1];
    }
}

int main(int argc, char *argv[]) {
    std::cin.tie(nullptr)->sync_with_stdio(false);

    std::string seedString;
    for (int i = 1; i < argc; ++i) {
        seedString += argv[i];
        seedString += " ";
    }
    rnd.seed(std::hash<std::string>{}(seedString));

    generate(argc, argv);

    return 0;
}
