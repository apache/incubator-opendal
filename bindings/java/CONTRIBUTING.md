# Contributing to Apache OpenDAL™ Java Bindings

This document covers building, testing, and styling the Java binding. For usage,
see the [user guide](https://opendal.apache.org/docs/bindings/java).

## Build

This project provides OpenDAL Java bindings with artifact name `opendal`. It depends on JDK 8 or later.

You can use Maven to build both the Rust dynamic lib and JAR files with one command:

```shell
./mvnw clean package -DskipTests=true
```

## Run tests

Currently, all tests are written in Java.

You can run the base tests with the following command:

```shell
./mvnw clean verify
```

## Code style

This project uses [spotless](https://github.com/diffplug/spotless) for code formatting so that all developers share a consistent code style without bikeshedding on it.

You can apply the code style with the following command:

```shell
./mvnw spotless:apply
```

## Run behavior tests

Services behavior tests read necessary configs from env vars or the `.env` file.

You can copy [.env.example](/.env.example) to `${project.rootdir}/.env` and change the values on need, or directly set env vars with `export KEY=VALUE`.

Take `fs` for example, we need to enable bench on `fs` on `/tmp/`:

```properties
OPENDAL_TEST=fs
OPENDAL_FS_ROOT=/tmp/
```

You can run service behavior tests of enabled services with the following command:

```shell
./mvnw test -Dtest="behavior.*Test"
```

## Classified library

Note that the dependency without classifier ships all classes and resources except the "opendal_java" shared library. And those with classifier bundle only the shared library.

For downstream usage, it's recommended:

* Depend on the one without classifier to write code;
* Depend on the classified ones with "test" for testing.

To load the shared library correctly, you can choose one of the following approaches:

* Append the classified JARs to the classpath at the runtime;
* Depend on the classified JARs and build a fat JAR (You may need to depend on all the provided classified JARs for running on multiple platforms);
* Build your own "opendal_java" shared library and specify "-Djava.library.path" to the folder containing that shared library.

The musl native library is dynamically linked against musl libc and the GCC runtime. On Alpine Linux, install `libgcc` before loading the native library.
</content>
