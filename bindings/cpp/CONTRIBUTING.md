# Contributing

- [Contributing](#contributing)
  - [Setup](#setup)
    - [Using a dev container environment](#using-a-dev-container-environment)
    - [Bring your own toolbox](#bring-your-own-toolbox)
  - [Build](#build)
  - [Test](#test)
  - [Documentation](#documentation)

## Setup

### Using a dev container environment

OpenDAL provides a pre-configured [dev container](https://containers.dev/) that could be used in [GitHub Codespaces](https://github.com/features/codespaces), [VSCode](https://code.visualstudio.com/), [JetBrains](https://www.jetbrains.com/remote-development/gateway/), [JuptyerLab](https://jupyterlab.readthedocs.io/en/stable/). Please pick up your favourite runtime environment.

The fastest way is:

[![Open in GitHub Codespaces](https://github.com/codespaces/badge.svg)](https://codespaces.new/apache/incubator-opendal?quickstart=1&machine=standardLinux32gb)

### Bring your own toolbox

To build OpenDAL C++ binding, the following is all you need:

- **C++ toolchain** that supports **c++17**, _e.g._ clang++ and g++

- **CMake**. It is used to generate the build system.

- **Ninja**. It is used to build the project. You can also use other build systems supported by CMake, _e.g._ make, bazel, etc. You just need to make corresponding changes to following commands.

- To format the code, you need to install **clang-format**

- **GTest(Google Test)**. It is used to run the tests. To see how to build, check [here](https://github.com/google/googletest).

- **Boost**. It is one dependency of this library. To see how to build, check [here](https://www.boost.org/doc/libs/1_76_0/more/getting_started/unix-variants.html).

- **Doxygen**. It is used to generate the documentation. To see how to build, check [here](https://www.doxygen.nl/manual/install.html).

- **Graphviz**. It is used to generate the documentation with graphs. To see how to build, check [here](https://graphviz.org/download/).

For Ubuntu and Debian:

```shell
# install C/C++ toolchain (can be replaced by other toolchains)
sudo apt install build-essential

# install CMake and Ninja
sudo apt install cmake ninja-build

# install clang-format
sudo apt install clang-format

# install GTest library
sudo apt-get install libgtest-dev

# install Boost library
sudo apt install libboost-all-dev

# install Doxygen and Graphviz
sudo apt install doxygen graphviz
```

For MacOS:

```shell
# install C/C++ toolchain (can be replaced by other toolchains)
xcode-select --install

# install CMake and Ninja
brew install cmake ninja

# install clang-format
brew install clang-format

# install GTest library
brew install googletest

# install Boost library
brew install boost

# install Doxygen and Graphviz
brew install doxygen graphviz
```

## Build

To build the library and header file.

```shell
mkdir build
cd build

# Add -DCMAKE_EXPORT_COMPILE_COMMANDS=1 to generate compile_commands.json for clangd
cmake -GNinja .. 

ninja
```

- The header file `opendal.hpp` is under `./include`.
- The library is under `build` after building.

To clean the build results.

```shell
ninja clean
```

## Test

To build and run the tests. (Note that you need to install GTest)

You should build the project first. Then run:

```shell
ninja test
```

## Documentation

To build the documentation. (Note that you need to install doxygen, graphviz)

```shell
ninja docs
```
