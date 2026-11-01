/*
 * Licensed to the Apache Software Foundation (ASF) under one
 * or more contributor license agreements.  See the NOTICE file
 * distributed with this work for additional information
 * regarding copyright ownership.  The ASF licenses this file
 * to you under the Apache License, Version 2.0 (the
 * "License"); you may not use this file except in compliance
 * with the License.  You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing,
 * software distributed under the License is distributed on an
 * "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
 * KIND, either express or implied.  See the License for the
 * specific language governing permissions and limitations
 * under the License.
 */

package org.apache.opendal.test;

import static org.assertj.core.api.Assertions.assertThat;
import java.nio.charset.StandardCharsets;
import java.util.HashMap;
import java.util.Map;
import org.apache.opendal.AsyncOperator;
import org.apache.opendal.Operator;
import org.junit.jupiter.api.Test;

public class OperatorDuplicateTest {

    @Test
    public void testDuplicateOperator() {
        final Map<String, String> conf = new HashMap<>();
        conf.put("root", "/opendal/");
        try (final AsyncOperator op = AsyncOperator.of("memory", conf)) {
            final String key = "key";
            final byte[] v0 = "v0".getBytes(StandardCharsets.UTF_8);
            final byte[] v1 = "v1".getBytes(StandardCharsets.UTF_8);

            try (final AsyncOperator duplicatedOp = op.duplicate()) {
                assertThat(duplicatedOp.info).isNotNull();
                assertThat(duplicatedOp.info).isEqualTo(op.info);
                duplicatedOp.write(key, v0).join();
                assertThat(duplicatedOp.read(key).join()).isEqualTo(v0);
            }

            assertThat(op.read(key).join()).isEqualTo(v0);
            op.write(key, v1).join();
            assertThat(op.read(key).join()).isEqualTo(v1);
        }
    }

    @Test
    public void testDuplicateBlockingOperator() {
        final Map<String, String> conf = new HashMap<>();
        conf.put("root", "/opendal/");
        try (final Operator op = Operator.of("memory", conf)) {
            final String key = "key";
            final byte[] v0 = "v0".getBytes(StandardCharsets.UTF_8);
            final byte[] v1 = "v1".getBytes(StandardCharsets.UTF_8);

            try (final Operator duplicatedOp = op.duplicate()) {
                assertThat(duplicatedOp.info).isNotNull();
                assertThat(duplicatedOp.info).isEqualTo(op.info);
                duplicatedOp.write(key, v0);
                assertThat(duplicatedOp.read(key)).isEqualTo(v0);
            }

            assertThat(op.read(key)).isEqualTo(v0);
            op.write(key, v1);
            assertThat(op.read(key)).isEqualTo(v1);
        }
    }
}
