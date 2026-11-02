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

import io.github.cdimascio.dotenv.Dotenv;
import io.github.cdimascio.dotenv.DotenvEntry;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.nio.file.Paths;
import java.util.HashMap;
import java.util.Map;
import java.util.UUID;
import lombok.Cleanup;
import lombok.extern.slf4j.Slf4j;
import org.apache.opendal.AsyncOperator;
import org.apache.opendal.Operator;
import org.apache.opendal.layer.CapabilityOverrideLayer;
import org.apache.opendal.layer.RetryLayer;
import org.junit.jupiter.api.extension.AfterAllCallback;
import org.junit.jupiter.api.extension.BeforeAllCallback;
import org.junit.jupiter.api.extension.ExtensionContext;
import org.junit.jupiter.api.extension.TestWatcher;

@Slf4j
public class BehaviorExtension implements BeforeAllCallback, AfterAllCallback, TestWatcher {
    private String testName;

    public String scheme;
    public AsyncOperator asyncOperator;
    public Operator operator;

    @Override
    public void beforeAll(ExtensionContext context) {
        final Dotenv dotenv = Dotenv.configure().ignoreIfMissing().load();
        final String scheme = dotenv.get("OPENDAL_TEST");
        if (scheme != null) {
            final Map<String, String> config = new HashMap<>();
            final String prefix = "opendal_" + scheme.toLowerCase() + "_";
            for (DotenvEntry entry : dotenv.entries()) {
                final String key = entry.getKey().toLowerCase();
                if (key.startsWith(prefix)) {
                    config.put(key.substring(prefix.length()), entry.getValue());
                }
            }

            // Isolate each test class unless OPENDAL_DISABLE_RANDOM_ROOT is set to true.
            if (!Boolean.parseBoolean(dotenv.get("OPENDAL_DISABLE_RANDOM_ROOT"))) {
                final String fixtureId = UUID.randomUUID().toString();
                final String root = config.getOrDefault("root", "/") + fixtureId + "/";
                config.put("root", root);
                if ("sled".equalsIgnoreCase(scheme)) {
                    final String datadir = config.get("datadir");
                    if (datadir != null && !datadir.isEmpty()) {
                        // Sled locks its physical database; logical roots do not isolate database handles.
                        config.put("datadir", Paths.get(datadir, fixtureId).toString());
                    }
                }
            }

            @Cleanup
            final AsyncOperator op = AsyncOperator.of(scheme.toLowerCase().replace('_', '-'), config);
            this.asyncOperator = op.layer(RetryLayer.builder().build());
            final String capabilityOverrides = dotenv.get("OPENDAL_TEST_CAPABILITY_OVERRIDES");
            if (capabilityOverrides != null && !capabilityOverrides.trim().isEmpty()) {
                this.asyncOperator = this.asyncOperator.layer(new CapabilityOverrideLayer(capabilityOverrides));
            }
            this.operator = this.asyncOperator.blocking();

            this.scheme = scheme;
            this.testName = String.format("%s(%s)", context.getDisplayName(), scheme);
            log.info(
                    "\n================================================================================"
                            + "\nTest {} is running."
                            + "\n--------------------------------------------------------------------------------",
                    testName);
        } else {
            log.warn(
                    "\n================================================================================"
                            + "\nTest {} is skipped because OPENDAL_TEST is not set."
                            + "\n--------------------------------------------------------------------------------",
                    context.getDisplayName());
        }
    }

    @Override
    public void afterAll(ExtensionContext context) {
        if (asyncOperator != null) {
            asyncOperator.close();
            asyncOperator = null;
        }

        if (operator != null) {
            operator.close();
            operator = null;
        }

        this.scheme = null;
        this.testName = null;
    }

    @Override
    public void testSuccessful(ExtensionContext context) {
        log.info(
                "\n================================================================================"
                        + "\nTest {}.{} successfully run."
                        + "\n--------------------------------------------------------------------------------",
                testName,
                context.getDisplayName());
    }

    @Override
    public void testFailed(ExtensionContext context, Throwable cause) {
        log.error(
                "\n================================================================================"
                        + "\nTest {}.{} failed with:\n{}"
                        + "\n--------------------------------------------------------------------------------",
                testName,
                context.getDisplayName(),
                exceptionToString(cause));
    }

    private static String exceptionToString(Throwable t) {
        if (t == null) {
            return "(null)";
        }

        try {
            StringWriter stm = new StringWriter();
            PrintWriter wrt = new PrintWriter(stm);
            t.printStackTrace(wrt);
            wrt.close();
            return stm.toString();
        } catch (Throwable ignored) {
            return t.getClass().getName() + " (error while printing stack trace)";
        }
    }
}
