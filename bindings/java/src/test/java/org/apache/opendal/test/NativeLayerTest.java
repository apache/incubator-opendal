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
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import org.apache.opendal.NativeLayer;
import org.apache.opendal.Operator;
import org.apache.opendal.layer.RetryNativeLayer;
import org.junit.jupiter.api.Test;

public class NativeLayerTest {
    @Test
    void testOperatorWithRetryLayer() {
        final Map<String, String> conf = new HashMap<>();
        conf.put("root", "/opendal/");
        final NativeLayer retryLayerSpec = RetryNativeLayer.builder().build();
        final List<NativeLayer> nativeLayerSpecs = Collections.singletonList(retryLayerSpec);
        try (final Operator op = Operator.of("memory", conf, nativeLayerSpecs)) {
            assertThat(op.info).isNotNull();
        }
    }
}
