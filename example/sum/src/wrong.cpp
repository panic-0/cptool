#include <iostream>

int main() {
    std::ios::sync_with_stdio(false);
    std::cin.tie(nullptr);

    int n;
    std::cin >> n;

    long long sum = 0;
    for (int i = 0; i < n; ++i) {
        int x;
        std::cin >> x;
        sum += x;
    }

    std::cout << sum << "\n";

    return 0;
}
