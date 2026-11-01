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

import static org.assertj.core.api.Assertions.assertThat;
import static org.junit.jupiter.api.Assumptions.assumeTrue;
import org.apache.opendal.Capability;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.condition.DisabledIfEnvironmentVariable;
import org.junit.jupiter.api.condition.EnabledIfEnvironmentVariable;
import org.junit.jupiter.api.extension.RegisterExtension;

@EnabledIfEnvironmentVariable(named = "OPENDAL_TEST", matches = "(?i)sled")
@DisabledIfEnvironmentVariable(named = "OPENDAL_DISABLE_RANDOM_ROOT", matches = "(?i)true")
class SledFixtureIsolationTest extends BehaviorTestBase {
    @RegisterExtension
    static final BehaviorExtension anotherFixture = new BehaviorExtension();

    @Test
    void testIndependentFixtures() {
        final Capability capability = op().info.capability;
        assumeTrue(capability.read && capability.write);

        final String path = "fixture-isolation";
        final byte[] first = {1};
        final byte[] second = {2};
        op().write(path, first);
        anotherFixture.operator.write(path, second);
        assertThat(op().read(path)).isEqualTo(first);
        assertThat(anotherFixture.operator.read(path)).isEqualTo(second);
        op().delete(path);
        anotherFixture.operator.delete(path);
    }
}
