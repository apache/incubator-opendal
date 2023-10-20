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
import java.util.Random;
import org.apache.opendal.BlockingOperator;
import org.apache.opendal.Operator;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.extension.RegisterExtension;

public abstract class BehaviorTestBase {
    @RegisterExtension
    public static final BehaviorExtension behaviorExtension = new BehaviorExtension();

    @BeforeAll
    public static void assume() {
        assumeTrue(behaviorExtension.operator != null);
        assumeTrue(behaviorExtension.blockingOperator != null);
    }

    protected Operator op() {
        return behaviorExtension.operator;
    }

    protected BlockingOperator blockingOp() {
        return behaviorExtension.blockingOperator;
    }

    /**
     * Generates a byte array of random content.
     */
    public static byte[] generateBytes() {
        final Random random = new Random();
        final int size = random.nextInt(4 * 1024 * 1024) + 1;
        final byte[] content = new byte[size];
        random.nextBytes(content);
        return content;
    }
}
