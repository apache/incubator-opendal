# OpenDAL Scripts

This module provides scripts to make maintainers lives easier.
OpenDAL users don't need to care about this folder.

NOTES: all scripts must be running at root folder of OpenDAL project.

## Release

```shell
just release
```

> Before running release, please make sure you have bump all versions.

### Preparations

Import gpg key

```shell
curl https://downloads.apache.org/opendal/KEYS > KEYS # Download KEYS
gpg --import KEYS # Import KEYS to local
```

Trust the public key

```shell
$ gpg --edit-key xxxxxxxxxx #KEY user used in this version
gpg (GnuPG) 2.2.21; Copyright (C) 2020 Free Software Foundation, Inc.
This is free software: you are free to change and redistribute it.
There is NO WARRANTY, to the extent permitted by law.

Secret key is available.
gpg> trust #trust
Please decide how far you trust this user to correctly verify other users' keys
(by looking at passports, checking fingerprints from different sources, etc.)

  1 = I don't know or won't say
  2 = I do NOT trust
  3 = I trust marginally
  4 = I trust fully
  5 = I trust ultimately
  m = back to the main menu

Your decision? 5 #choose 5
Do you really want to set this key to ultimate trust? (y/N) y  #choose y
```

## Verify

```shell
./scripts/verify.py
```
