#include <iostream>
#include <sys/mount.h>
#include <cstddef>
#include <errno.h>

int main() {
    auto ret = mount("/tmp", "/var/empty", nullptr, MS_BIND | MS_RDONLY, nullptr);
    std::cout << "Ret: " << ret << std::endl;
    if (ret == -1) {
        perror("Failed");
        return 1;
    }

    return 0;
}
