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
import java.lang.reflect.Field;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.List;
import java.util.Locale;
import java.util.stream.Collectors;
import org.apache.opendal.AsyncOperator;
import org.apache.opendal.Capability;
import org.apache.opendal.Operator;
import org.apache.opendal.OperatorInfo;
import org.apache.opendal.ServiceConfig;
import org.apache.opendal.layer.CapabilityOverrideLayer;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.ValueSource;

public class OperatorInfoTest {

    @TempDir
    private static Path tempDir;

    @Test
    public void testBlockingOperatorInfo() {
        final ServiceConfig.Fs fs =
                ServiceConfig.Fs.builder().root(tempDir.toString()).build();

        try (final Operator op = Operator.of(fs)) {
            final OperatorInfo info = op.info;
            assertThat(info).isNotNull();
            assertThat(info.scheme).isEqualTo("fs");

            assertThat(info.capability).isNotNull();
            assertThat(info.capability.read).isTrue();
            assertThat(info.capability.write).isTrue();
            assertThat(info.capability.delete).isTrue();
            assertThat(info.capability.writeCanAppend).isTrue();
            assertThat(info.capability.writeMultiMaxSize).isEqualTo(-1);
            assertThat(info.capability.writeMultiMinSize).isEqualTo(-1);
        }
    }

    @Test
    public void testOperatorInfo() {
        final ServiceConfig.Memory memory =
                ServiceConfig.Memory.builder().root("/opendal/").build();
        try (final AsyncOperator op = AsyncOperator.of(memory)) {
            final OperatorInfo info = op.info;
            assertThat(info).isNotNull();
            assertThat(info.scheme).isEqualTo("memory");

            assertThat(info.capability).isNotNull();
            assertThat(info.capability.read).isTrue();
            assertThat(info.capability.write).isTrue();
            assertThat(info.capability.delete).isTrue();
            assertThat(info.capability.writeCanAppend).isFalse();
            assertThat(info.capability.writeMultiMaxSize).isEqualTo(-1);
            assertThat(info.capability.writeMultiMinSize).isEqualTo(-1);
        }
    }

    @Test
    public void testBooleanCapabilityMappings() throws Exception {
        List<Field> flags = Arrays.stream(Capability.class.getFields())
                .filter(field -> field.getType() == boolean.class)
                .collect(Collectors.toList());
        String allDisabled =
                flags.stream().map(field -> capabilityName(field) + "=false").collect(Collectors.joining(","));
        try (AsyncOperator base =
                        AsyncOperator.of(ServiceConfig.Memory.builder().build());
                AsyncOperator none = base.layer(new CapabilityOverrideLayer(allDisabled))) {
            // Enable one flag at a time so swapped fields cannot pass an all-true check.
            for (Field enabledField : flags) {
                String key = capabilityName(enabledField);
                try (AsyncOperator enabled = none.layer(new CapabilityOverrideLayer(key + "=true"));
                        AsyncOperator disabled = enabled.layer(new CapabilityOverrideLayer(key + "=false"))) {
                    for (Field field : flags) {
                        assertThat(field.getBoolean(enabled.info.capability))
                                .as("%s when enabling %s", field.getName(), key)
                                .isEqualTo(field.equals(enabledField));
                        assertThat(field.getBoolean(disabled.info.capability))
                                .as("%s after disabling %s", field.getName(), key)
                                .isFalse();
                    }
                }
            }
        }
    }

    @ParameterizedTest
    @ValueSource(longs = {-1, 0, 4294967296L, Long.MAX_VALUE})
    public void testCapabilityLimitMappings(long value) throws Exception {
        List<Field> limits = Arrays.stream(Capability.class.getFields())
                .filter(field -> field.getType() == long.class)
                .collect(Collectors.toList());
        StringBuilder overrides = new StringBuilder();
        long[] expected = new long[limits.size()];
        for (int i = 0; i < limits.size(); i++) {
            if (i > 0) {
                overrides.append(',');
            }
            expected[i] = value <= 0 ? value : value - i;
            overrides
                    .append(capabilityName(limits.get(i)))
                    .append('=')
                    .append(value == -1 ? "none" : Long.toString(expected[i]));
        }
        try (AsyncOperator base =
                        AsyncOperator.of(ServiceConfig.Memory.builder().build());
                AsyncOperator op = base.layer(new CapabilityOverrideLayer(overrides.toString()))) {
            for (int i = 0; i < limits.size(); i++) {
                Field field = limits.get(i);
                assertThat(field.getLong(op.info.capability))
                        .as(field.getName())
                        .isEqualTo(expected[i]);
            }
        }
    }

    private static String capabilityName(Field field) {
        return field.getName().replaceAll("([a-z0-9])([A-Z])", "$1_$2").toLowerCase(Locale.ROOT);
    }
}
