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

package org.apache.opendal.test.behavior;

import static org.junit.jupiter.api.Assumptions.assumeTrue;
import java.util.UUID;
import org.apache.opendal.OperatorOutputStream;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestInstance;

@TestInstance(TestInstance.Lifecycle.PER_CLASS)
public class RegressionTest extends BehaviorTestBase {
    // Modify the value of ITERATIONS to do stress testing.
    private static final int ITERATIONS = 10;

    // @see https://github.com/apache/opendal/issues/5421
    @Test
    public void testAzblobLargeFile() throws Exception {
        assumeTrue(scheme() != null && scheme().equalsIgnoreCase("azblob"));

        final String path = UUID.randomUUID().toString();
        final int size = 16384 * 10; // 10 x OperatorOutputStream.DEFAULT_MAX_BYTES (10 flushes per write)
        final byte[] content = generateBytes(size);

        try (OperatorOutputStream operatorOutputStream = op().createOutputStream(path, size)) {
            for (int i = 0; i < ITERATIONS; i++) {
                // More iterations in case BlockCountExceedsLimit doesn't pop up exactly after 100K blocks.
                operatorOutputStream.write(content);
            }
        }
    }
}
