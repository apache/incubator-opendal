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
import static org.assertj.core.api.Assertions.assertThatThrownBy;
import java.util.UUID;
import org.apache.opendal.OpenDALException;
import org.apache.opendal.Operator;
import org.apache.opendal.ServiceConfig;
import org.apache.opendal.StatOptions;
import org.apache.opendal.test.condition.OpenDALExceptionCondition;
import org.junit.jupiter.api.Test;

public class StatOptionsTest {

    /**
     * The memory service supports neither version condition, so core rejects each one as
     * unsupported. A condition dropped by the binding would let the stat succeed instead.
     */
    @Test
    public void testVersionConditionsReachCore() {
        final ServiceConfig.Memory memory =
                ServiceConfig.Memory.builder().root("/opendal/").build();
        try (final Operator op = Operator.of(memory)) {
            assertThat(op.info.capability.statWithIfVersionMatch).isFalse();
            assertThat(op.info.capability.statWithIfVersionNotMatch).isFalse();

            final String path = UUID.randomUUID().toString();
            op.write(path, "content");

            final StatOptions ifVersionMatch =
                    StatOptions.builder().ifVersionMatch("version").build();
            assertThatThrownBy(() -> op.stat(path, ifVersionMatch))
                    .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.Unsupported));

            final StatOptions ifVersionNotMatch =
                    StatOptions.builder().ifVersionNotMatch("version").build();
            assertThatThrownBy(() -> op.stat(path, ifVersionNotMatch))
                    .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.Unsupported));
        }
    }
}
