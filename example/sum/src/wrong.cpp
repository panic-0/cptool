#include <iostream>

constexpr long long P = 998244353;

int main() {
    std::ios::sync_with_stdio(false);
    std::cin.tie(nullptr);

    int n, k;
    std::cin >> n >> k;
    (void)k;

    long long sum = 0;
    for (int i = 0; i < n; ++i) {
        long long x;
        std::cin >> x;
        sum = (sum + x) % P;
    }

    std::cout << sum << "\n";

    return 0;
}
