# Contributing
- [Contributing](#contributing)
   - [Setup](#setup)
      - [Using a dev container environment](#using-a-dev-container-environment)
      - [Bring your own toolbox](#bring-your-own-toolbox)
   - [Build](#build)
   - [Test](#test)
   - [Docs](#docs)
   - [Misc](#misc)

## Setup

### Using a dev container environment
OpenDAL provides a pre-configured [dev container](https://containers.dev/) that could be used in [Github Codespaces](https://github.com/features/codespaces), [VSCode](https://code.visualstudio.com/), [JetBrains](https://www.jetbrains.com/remote-development/gateway/), [JuptyerLab](https://jupyterlab.readthedocs.io/en/stable/). Please pick up your favourite runtime environment.

The fastest way is:

[![Open in GitHub Codespaces](https://github.com/codespaces/badge.svg)](https://codespaces.new/apache/incubator-opendal?quickstart=1&machine=standardLinux32gb)

### Bring your own toolbox
To build OpenDAL C binding, the following is all you need:
- **A C++ compiler** that supports **c++14**, *e.g.* clang++ and g++

- To format the code, you need to install **clang-format**
    - The `opendal.h` is not formatted by hands when you contribute, please do not format the file. **Use `make format` only.**
    - If your contribution is related to the files under `./tests`, you may format it before submitting your pull request. But notice that different versions of `clang-format` may format the files differently.

- **GTest(Google Test)** need to be installed to build the BDD (Behavior Driven Development) tests. To see how to build, check [here](https://github.com/google/googletest).

For Ubuntu and Debian:
```shell
# install C/C++ toolchain
sudo apt install -y build-essential

# install clang-format
sudo apt install clang-format

# install and build GTest library under /usr/lib and softlink to /usr/local/lib
sudo apt-get install libgtest-dev
cd /usr/src/gtest
sudo cmake CMakeLists.txt
sudo make
sudo cp lib/*.a /usr/lib
sudo ln -s /usr/lib/libgtest.a /usr/local/lib/libgtest.a
sudo ln -s /usr/lib/libgtest_main.a /usr/local/lib/libgtest_main.a
```

## Build
To build the library and header file.
```shell
make build
```

- The header file `opendal.h` is under `./include` 
- The library is under `../../target/debug` after building.

To clean the build results.
```shell
make clean
```

## Test
To build and run the tests. (Note that you need to install GTest)
```shell
make test
```

```text
[==========] Running 1 test from 1 test suite.
[----------] Global test environment set-up.
[----------] 1 test from OpendalBddTest
[ RUN      ] OpendalBddTest.FeatureTest
[       OK ] OpendalBddTest.FeatureTest (0 ms)
[----------] 1 test from OpendalBddTest (0 ms total)

[----------] Global test environment tear-down
[==========] 1 test from 1 test suite ran. (0 ms total)
[  PASSED  ] 1 test.
```

