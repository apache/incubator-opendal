## Compatible Services

### OpenStack Swift

[OpenStack Swift](https://docs.openstack.org/swift/latest/) is the default implementations of swift services.

To connect to OpenStack Swift, we need to set:

- `endpoint`: The endpoint of OpenStack Swift, for example: `http://127.0.0.1:8080/v1/AUTH_test`.
- `container`: The name of OpenStack Swift container.
- `token`: OpenStack Swift container personal access token.

```rust,ignore
builder.endpoint("http://127.0.0.1:8080/v1/AUTH_test");
builder.container("container");
builder.token("token");
```

`endpoint` is the full URL that serves as the access point to all containers under an OpenStack Swift account. It represents the entry point for accessing the resources of the account. Alongside `endpoint`, `token` is used as a credential to verify the user's identity and authorize access to the relevant resources. Both `endpoint` and `token` can be obtained through OpenStack Swift authentication service.

`endpoint` consists of server address and port, API version, authenticated account ID. For instance, it might appear as follows:

- `http://127.0.0.1:8080/v1/AUTH_test`.
- `http://192.168.66.88:8080/swift/v1`.
- `https://openstack-controller.example.com:8080/v1/account`.

Please note that the exact format of `endpoint` may vary depending on the deployment configuration and version of swift services. Users can refer to the specific services documentation for the correct `endpoint` format and authentication method.

For more information, refer:

- [OpenStack Swift API](https://docs.openstack.org/api-ref/object-store/).
- [OpenStack Swift Authentication](https://docs.openstack.org/swift/latest/api/object_api_v1_overview.html).

### Ceph Rados Gateway

[Ceph Rados Gateway](https://docs.ceph.com/en/quincy/radosgw/) supports a RESTful API that is compatible with the basic data access model of OpenStack Swift API.

To connect to Ceph Rados Gateway, we need to set:

- `endpoint`: The endpoint of swift services, for example: `http://127.0.0.1:8080/swift/v1`.
- `container`: The name of swift container.
- `token`: swift container personal access token.

```rust,ignore
builder.endpoint("http://127.0.0.1:8080/swift/v1");
builder.container("container");
builder.token("token");
```

For more information, refer:

- [Ceph Rados Gateway Swift API](https://docs.ceph.com/en/latest/radosgw/swift/#api).
- [Ceph Rados Gateway Swift Authentication](https://docs.ceph.com/en/latest/radosgw/swift/auth/).
