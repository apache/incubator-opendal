#! /bin/bash
# Licensed to the Apache Software Foundation (ASF) under one
# or more contributor license agreements.  See the NOTICE file
# distributed with this work for additional information
# regarding copyright ownership.  The ASF licenses this file
# to you under the Apache License, Version 2.0 (the
# "License"); you may not use this file except in compliance
# with the License.  You may obtain a copy of the License at
#
#   http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing,
# software distributed under the License is distributed on an
# "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
# KIND, either express or implied.  See the License for the
# specific language governing permissions and limitations
# under the License.

set +e

mkdir -p /tmp/tikv/ssl

cd /tmp/tikv/ssl

cat << EOF > ca-csr.json
{
    "CN": "Opendal Test CA",
    "key": {
        "algo": "rsa",
        "size": 2048
    },
    "names": [
        {
            "C": "CN",
            "L": "BJ",
	        "O": "Opendal",
            "ST": "Beijing"
        }
    ]
}
EOF

cat << EOF > ca-config.json
{
    "signing": {
        "default": {
            "expiry": "43800h"
        },
        "profiles": {
            "server": {
                "expiry": "43800h",
                "usages": [
                    "signing",
                    "key encipherment",
                    "server auth",
		            "client auth"
                ]
            },
            "client": {
                "expiry": "43800h",
                "usages": [
                    "signing",
                    "key encipherment",
                    "client auth"
                ]
            }
        }
    }
}
EOF

curl -L -o cfssl https://pkg.cfssl.org/R1.2/cfssl_linux-amd64

curl -L -o cfssljson https://pkg.cfssl.org/R1.2/cfssljson_linux-amd64

chmod +x cfssl*

./cfssl gencert -initca ca-csr.json | ./cfssljson -bare ca -

echo '{"CN":"tikv-server","hosts":[""],"key":{"algo":"rsa","size":2048}}' | ./cfssl gencert -ca=ca.pem -ca-key=ca-key.pem -config=ca-config.json -profile=server -hostname="127.0.0.1" - | ./cfssljson -bare tikv-server

echo '{"CN":"pd-server","hosts":[""],"key":{"algo":"rsa","size":2048}}' | ./cfssl gencert -ca=ca.pem -ca-key=ca-key.pem -config=ca-config.json -profile=server -hostname="127.0.0.1" - | ./cfssljson -bare pd-server

echo '{"CN":"client","hosts":[""],"key":{"algo":"rsa","size":2048}}' | ./cfssl gencert -ca=ca.pem -ca-key=ca-key.pem -config=ca-config.json -profile=client -hostname="" - | ./cfssljson -bare client
