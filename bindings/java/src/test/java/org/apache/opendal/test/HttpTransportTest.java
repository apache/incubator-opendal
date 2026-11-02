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
import com.sun.net.httpserver.HttpServer;
import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import org.apache.opendal.AsyncExecutor;
import org.apache.opendal.AsyncOperator;
import org.apache.opendal.Operator;
import org.apache.opendal.ServiceConfig;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.Timeout;

public class HttpTransportTest {
    @Test
    @Timeout(10)
    void testDefaultHttpTransport() throws Exception {
        final byte[] content = "Hello, OpenDAL!".getBytes(StandardCharsets.UTF_8);
        final HttpServer server = HttpServer.create(new InetSocketAddress("127.0.0.1", 0), 0);
        server.createContext("/example.txt", exchange -> {
            try {
                exchange.sendResponseHeaders(200, content.length);
                exchange.getResponseBody().write(content);
            } finally {
                exchange.close();
            }
        });
        server.start();
        try {
            final ServiceConfig.Http config = ServiceConfig.Http.builder()
                    .endpoint("http://127.0.0.1:" + server.getAddress().getPort())
                    .build();
            try (final AsyncExecutor executor = AsyncExecutor.createTokioExecutor(1);
                    final AsyncOperator async = AsyncOperator.of(config, executor);
                    final Operator blocking = async.blocking()) {
                assertThat(async.read("example.txt").join()).isEqualTo(content);
                assertThat(blocking.read("example.txt")).isEqualTo(content);
            }
        } finally {
            server.stop(0);
        }
    }
}
