#include <iostream>

constexpr int P = 998244353;

int main() {
    int n;
    std::cin >> n;

    int sum = 0;
    for (int i = 0; i < n; ++i) {
        int x;
        std::cin >> x;
        sum = (sum + x) % P;
    }

    std::cout << sum << "\n";

    return 0;
}
