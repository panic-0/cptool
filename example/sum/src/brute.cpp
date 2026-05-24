#include <iostream>
#include <vector>

constexpr long long P = 998244353;

int main() {
    std::ios::sync_with_stdio(false);
    std::cin.tie(nullptr);

    int n, k;
    std::cin >> n >> k;

    std::vector<long long> a(n);
    for (long long &x : a) {
        std::cin >> x;
    }

    long long ans = 0;
    for (long long x : a) {
        long long cur = 1;
        x %= P;
        for (int p = 1; p <= k; ++p) {
            cur = cur * x % P;
            ans += cur;
            ans %= P;
        }
    }

    std::cout << ans << "\n";

    return 0;
}
