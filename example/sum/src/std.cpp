#include <iostream>

constexpr long long P = 998244353;

long long mod_pow(long long a, long long e) {
    long long r = 1;
    while (e > 0) {
        if (e & 1) {
            r = r * a % P;
        }
        a = a * a % P;
        e >>= 1;
    }
    return r;
}

long long power_sum(long long a, int k) {
    a %= P;
    if (a == 1) {
        return k % P;
    }
    long long numerator = (mod_pow(a, k) - 1 + P) % P;
    long long denominator = (a - 1 + P) % P;
    return a * numerator % P * mod_pow(denominator, P - 2) % P;
}

int main() {
    std::ios::sync_with_stdio(false);
    std::cin.tie(nullptr);

    int n, k;
    std::cin >> n >> k;

    long long ans = 0;
    for (int i = 0; i < n; ++i) {
        long long x;
        std::cin >> x;
        ans += power_sum(x, k);
        ans %= P;
    }

    std::cout << ans << "\n";

    return 0;
}
