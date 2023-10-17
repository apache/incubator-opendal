---
title: Download
---

# Download

The official Apache OpenDAL (incubating) releases are as source artifacts.

## Releases

| Name              | Archive                                                                                                  | Signature                                                                                                | Checksum                                                                                                       |
|-------------------|----------------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------|
| 0.41.0-incubating | [tarball](https://dlcdn.apache.org/incubator/opendal/0.41.0/apache-opendal-incubating-0.41.0-src.tar.gz) | [asc](https://dlcdn.apache.org/incubator/opendal/0.41.0/apache-opendal-incubating-0.41.0-src.tar.gz.asc) | [sha512](https://dlcdn.apache.org/incubator/opendal/0.41.0/apache-opendal-incubating-0.41.0-src.tar.gz.sha512) |
| 0.40.0-incubating | [tarball](https://dlcdn.apache.org/incubator/opendal/0.40.0/apache-opendal-incubating-0.40.0-src.tar.gz) | [asc](https://dlcdn.apache.org/incubator/opendal/0.40.0/apache-opendal-incubating-0.40.0-src.tar.gz.asc) | [sha512](https://dlcdn.apache.org/incubator/opendal/0.40.0/apache-opendal-incubating-0.40.0-src.tar.gz.sha512) |
| 0.39.0-incubating | [tarball](https://dlcdn.apache.org/incubator/opendal/0.39.0/apache-opendal-incubating-0.39.0-src.tar.gz) | [asc](https://dlcdn.apache.org/incubator/opendal/0.39.0/apache-opendal-incubating-0.39.0-src.tar.gz.asc) | [sha512](https://dlcdn.apache.org/incubator/opendal/0.39.0/apache-opendal-incubating-0.39.0-src.tar.gz.sha512) |
| 0.38.1-incubating | [tarball](https://dlcdn.apache.org/incubator/opendal/0.38.1/apache-opendal-incubating-0.38.1-src.tar.gz) | [asc](https://dlcdn.apache.org/incubator/opendal/0.38.1/apache-opendal-incubating-0.38.1-src.tar.gz.asc) | [sha512](https://dlcdn.apache.org/incubator/opendal/0.38.1/apache-opendal-incubating-0.38.1-src.tar.gz.sha512) |
| 0.38.0-incubating | [tarball](https://dlcdn.apache.org/incubator/opendal/0.38.0/apache-opendal-incubating-0.38.0-src.tar.gz) | [asc](https://dlcdn.apache.org/incubator/opendal/0.38.0/apache-opendal-incubating-0.38.0-src.tar.gz.asc) | [sha512](https://dlcdn.apache.org/incubator/opendal/0.38.0/apache-opendal-incubating-0.38.0-src.tar.gz.sha512) |
| 0.37.0-incubating | [tarball](https://dlcdn.apache.org/incubator/opendal/0.37.0/apache-opendal-incubating-0.37.0-src.tar.gz) | [asc](https://dlcdn.apache.org/incubator/opendal/0.37.0/apache-opendal-incubating-0.37.0-src.tar.gz.asc) | [sha512](https://dlcdn.apache.org/incubator/opendal/0.37.0/apache-opendal-incubating-0.37.0-src.tar.gz.sha512) |
| 0.36.0-incubating | [tarball](https://dlcdn.apache.org/incubator/opendal/0.36.0/apache-opendal-incubating-0.36.0-src.tar.gz) | [asc](https://dlcdn.apache.org/incubator/opendal/0.36.0/apache-opendal-incubating-0.36.0-src.tar.gz.asc) | [sha512](https://dlcdn.apache.org/incubator/opendal/0.36.0/apache-opendal-incubating-0.36.0-src.tar.gz.sha512) |

For older releases, please check the [archive](https://dlcdn.apache.org/incubator/opendal/).

## Notes

* When downloading a release, please check the SHA-512 and verify the OpenPGP compatible signature from the main Apache site. Links are provided above (next to the release download link).
* The KEYS file contains the public keys used for signing release. It is recommended that (when possible) a web of trust is used to confirm the identity of these keys.
* Please download the [KEYS](https://downloads.apache.org/incubator/opendal/KEYS) as well as the .asc signature files.

### To verify the signature of the release artifact

You will need to download both the release artifact and the .asc signature file for that artifact. Then verify the signature by:

* Download the KEYS file and the .asc signature files for the relevant release artifacts.
* Import the KEYS file to your GPG keyring: 

    ```shell
    gpg --import KEYS
    ```

* Verify the signature of the release artifact using the following command:
  
    ```shell
    gpg --verify <artifact>.asc <artifact>
    ```

### To verify the checksum of the release artifact

You will need to download both the release artifact and the .sha512 checksum file for that artifact. Then verify the checksum by:

```shell
shasum -a 512 -c <artifact>.sha512
```
