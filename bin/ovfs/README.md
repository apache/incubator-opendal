# OpenDAL File System via Virtio (WIP)

OpenDAL File System via Virtio (ovfs) is a backend implementation of VirtioFS. It provides a file system interface for VMs based on OpenDAL, aiming to accelerate VMs IO performance by VirtIO and seamlessly connect with various storage backends.

Note that this project is still under development and is not yet ready to run.

## How to Use

The following components are required:
- Rust environment on the host to run ovfs
- QEMU 4.2 or later for built-in virtiofs support
- A Linux 5.4 or later guest kernel for built-in virtiofs support

### Install QEMU and VMs

```shell
sudo apt-get -y qemu qemu-kvm # debian/ubuntu
```

Download and install the VM, taking Ubuntu as an example:

```shell
wget https://releases.ubuntu.com/20.04/ubuntu-20.04.6-live-server-amd64.iso
qemu-img create -f qcow2 ubuntu.qcow2 20G
qemu-system-x86_64 -enable-kvm -smp 8 -m 16G -cdrom ubuntu-20.04.6-live-server-amd64.iso -drive file=ubuntu.qcow2,format=qcow2 -boot d
```

### Mount shared directory on VMs

Run ovfs and set the listening socket path and service configuration used:

```shell
host# cargo run --release <socket-path> <backend-url>
```

backend-url is the URL that includes the scheme and parameters of the service used, in the following format:
- fs://?root=<directory>
- s3://?root=<path>&bucket=<bucket>&endpoint=<endpoint>&region=<region>&access_key_id=<access-key-id>&secret_access_key=<secret-access-key>

Run the VM through QEMU and create a virtiofs device:

```shell
host# qemu-system-x86_64 -M pc -cpu host --enable-kvm -smp 8 \
     -m 16G -object memory-backend-file,id=mem,size=4G,mem-path=/dev/shm,share=on -numa node,memdev=mem \
     -chardev socket,id=char0,path=<socket-path> -device vhost-user-fs-pci,queue-size=1024,chardev=char0,tag=<fs-tag> \
     -chardev stdio,mux=on,id=mon -mon chardev=mon,mode=readline -device virtio-serial-pci -device virtconsole,chardev=mon -vga none -display none \
     -drive if=virtio,file=ubuntu.qcow2
```

Mount a shared directory in the VM:

```shell
guest# sudo mount -t virtiofs <fs-tag> <mount-point>
```

## License and Trademarks

Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0

Apache OpenDAL, OpenDAL, and Apache are either registered trademarks or trademarks of the Apache Software Foundation.
