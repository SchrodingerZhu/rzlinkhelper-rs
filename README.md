You can set the config via environment variable `RZ_CONFIG`. If not set,
the helper will seek the config file at the work dir.
Sample config
```json5
{
    "callpass_library_path": "./libcallpass.so",
    "original_cxx_executable": "/usr/bin/c++",
    "original_cc_executable": "/usr/bin/cc",
    "targeted_cxx_executable": "/usr/bin/clang++",
    "targeted_cc_executable": "/usr/bin/clang",
    "llvm_link_executable": "/usr/bin/llvm-link",
    "cmaker_executable": "/home/schrodinger/CLionProject/cmaker/target/release/cmaker",
    "cmake_executable": "/usr/bin/cmake",
    "remake_executable": "/usr/bin/remake"
}
```