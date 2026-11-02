---
title: Verify a release candidate
sidebar_position: 4
---

Use the following checklist to verify a release candidate:

- [ ] Download links work.
- [ ] Checksums and signatures are valid.
- [ ] LICENSE and NOTICE files are present.
- [ ] Source packages contain no unexpected binary files.
- [ ] Source files include ASF license headers.
- [ ] Source builds successfully.

:::note

You do not need to complete every check before casting a vote for a release candidate.

Clearly state which checks you performed. The release manager uses this information to ensure that the verification covers every check.

:::

## Download links work

Download the release candidate source packages from the [dist](https://dist.apache.org/repos/dist/dev/opendal/) directory.

OpenDAL distributes each release candidate as a directory of split source packages. Download the entire release candidate (RC) directory with SVN.

Replace `${release_version}` with the RC version, such as `0.55.0-rc.1`:

```shell
svn checkout https://dist.apache.org/repos/dist/dev/opendal/${release_version}/ opendal-dist-${release_version}
```

## Checksums and signatures are valid

Every source archive in a release candidate has a corresponding checksum and signature file.

For example, the `0.55.0-rc.1` directory contains files with names such as:

```
https://dist.apache.org/repos/dist/dev/opendal/0.55.0-rc.1/apache-opendal-core-0.55.0-src.tar.gz.sha512
https://dist.apache.org/repos/dist/dev/opendal/0.55.0-rc.1/apache-opendal-core-0.55.0-src.tar.gz.asc
https://dist.apache.org/repos/dist/dev/opendal/0.55.0-rc.1/apache-opendal-bindings-java-0.48.2-src.tar.gz.sha512
https://dist.apache.org/repos/dist/dev/opendal/0.55.0-rc.1/apache-opendal-bindings-java-0.48.2-src.tar.gz.asc
```

The RC directory uses the release candidate version, while each source archive uses its package-specific version.
Do not expect a single `apache-opendal-${opendal_version}-src.tar.gz` source archive or any `apache-opendal-bin-*` artifacts in this repository.

### Verify checksums and signatures

Use GnuPG to verify signatures. Install it with one of the following commands:

```shell
apt-get install gnupg
# or
yum install gnupg
# or
brew install gnupg
```

First, import the OpenDAL release manager's public key:

```shell
curl https://downloads.apache.org/opendal/KEYS > KEYS # Download KEYS
gpg --import KEYS # Import KEYS to local
```

Next, trust the public key:

```shell
gpg --edit-key <KEY-used-in-this-version> # Edit the key
```

GnuPG opens an interactive session. Enter the following command to trust the key:

```shell
gpg> trust
```

Then select a trust level. For example:

```
Please decide how far you trust this user to correctly verify other users' keys
(by looking at passports, checking fingerprints from different sources, etc.)

  1 = I don't know or won't say
  2 = I do NOT trust
  3 = I trust marginally
  4 = I trust fully
  5 = I trust ultimately
  m = back to the main menu
```

Select `5` to trust the key ultimately.

You can now verify the release candidate.

OpenDAL provides a script that verifies the checksums and signatures of the release candidate source packages.

Download the script from the RC tag into the release candidate directory:

```shell
cd opendal-dist-${release_version}
curl --silent --show-error --location https://github.com/apache/opendal/raw/v${release_version}/scripts/verify.py --output verify.py
```

The script checks every `*.tar.gz` in the RC directory that has matching `.asc` and `.sha512` files, extracts each `apache-opendal-*-src` tree, verifies `LICENSE` and `NOTICE`, builds `core`, and builds `bindings/java` when that package is present.

The script streams Cargo and Maven output to the terminal so you can follow build progress and inspect errors.

Run the script:

```shell
python ./verify.py
```

You will see output similar to the following if the verification is successful (build logs omitted):

```shell
$ python ./verify.py
> Checking apache-opendal-core-0.55.0-src.tar.gz
gpg: Signature made Fri Jun  7 20:57:06 2024 CST
gpg:                using RSA key 8B374472FAD328E17F479863B379691FC6E298DD
gpg: Good signature from "Zili Chen (CODE SIGNING KEY) <tison@apache.org>" [unknown]
gpg: WARNING: This key is not certified with a trusted signature!
gpg:          There is no indication that the signature belongs to the owner.
Primary key fingerprint: 8B37 4472 FAD3 28E1 7F47  9863 B379 691F C6E2 98DD
> Success to verify the gpg sign for apache-opendal-core-0.55.0-src.tar.gz
apache-opendal-core-0.55.0-src.tar.gz: OK
> Success to verify the checksum for apache-opendal-core-0.55.0-src.tar.gz
> Checking apache-opendal-bindings-java-0.48.2-src.tar.gz
apache-opendal-bindings-java-0.48.2-src.tar.gz: OK
> Success to verify the checksum for apache-opendal-bindings-java-0.48.2-src.tar.gz
.......
> Start checking LICENSE file in /Users/yan/Downloads/opendal-dev/apache-opendal-core-0.55.0-src
> LICENSE file exists in /Users/yan/Downloads/opendal-dev/apache-opendal-core-0.55.0-src
> Start checking NOTICE file in /Users/yan/Downloads/opendal-dev/apache-opendal-core-0.55.0-src
> NOTICE file exists in /Users/yan/Downloads/opendal-dev/apache-opendal-core-0.55.0-src
cargo 1.78.0 (54d8815d0 2024-03-26)
Start building opendal core
Success to build opendal core
openjdk version "22.0.1" 2024-04-16
OpenJDK Runtime Environment Temurin-22.0.1+8 (build 22.0.1+8)
OpenJDK 64-Bit Server VM Temurin-22.0.1+8 (build 22.0.1+8, mixed mode)
Start building opendal java binding
> Success to build opendal java binding
```

## Verify source package contents

Unpack each release candidate source package, such as `apache-opendal-core-0.55.0-src.tar.gz` or `apache-opendal-bindings-java-0.48.2-src.tar.gz`, and verify the following:

- Package layout matches the package being released.
- Required repository-local dependencies are included. For example, binding and integration packages include `core`.
- LICENSE and NOTICE files are present and correct.
- Source packages contain no unexpected binary files.
- Source files include ASF license headers where required.
- Source builds successfully.

## Verify OpenDAL Java Maven artifacts

Download the artifacts from `https://repository.apache.org/content/repositories/orgapacheopendal-${maven_artifact_number}/`.

Verify the following:

- JAR checksums match the bundled checksum files.
- JAR signatures match the bundled signature files.
- JARs are reproducible locally. Build the JARs on your machine and verify that their checksums match the bundled checksums.

Reproducing the artifacts requires the same JDK and Maven distributions. Use [Eclipse Temurin JDK 8](https://adoptium.net/temurin/releases/?version=8) and the bundled Maven Wrapper to reproduce the artifacts.
