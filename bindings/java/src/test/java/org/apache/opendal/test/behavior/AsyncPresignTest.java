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

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assumptions.assumeTrue;
import java.io.IOException;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.time.Duration;
import java.util.Map;
import java.util.UUID;
import org.apache.hc.client5.http.impl.classic.CloseableHttpClient;
import org.apache.hc.client5.http.impl.classic.HttpClients;
import org.apache.hc.core5.http.ClassicHttpResponse;
import org.apache.hc.core5.http.HttpHeaders;
import org.apache.hc.core5.http.HttpStatus;
import org.apache.hc.core5.http.io.entity.EntityUtils;
import org.apache.hc.core5.http.io.support.ClassicRequestBuilder;
import org.apache.opendal.Capability;
import org.apache.opendal.Metadata;
import org.apache.opendal.PresignedRequest;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestInstance;

@TestInstance(TestInstance.Lifecycle.PER_CLASS)
public class AsyncPresignTest extends BehaviorTestBase {

    @BeforeAll
    public void precondition() {
        final Capability capability = asyncOp().info.capability;
        assumeTrue(capability.list && capability.write && capability.presign);
    }

    /**
     * Presign write should succeed.
     */
    @Test
    public void testPresignWrite() throws IOException {
        final String path = UUID.randomUUID().toString();
        final byte[] content = generateBytes();

        final PresignedRequest signedReq =
                asyncOp().presignWrite(path, Duration.ofSeconds(3600)).join();

        try (CloseableHttpClient httpclient = HttpClients.createDefault()) {
            final ClassicRequestBuilder builder =
                    createRequestBuilder(signedReq).setEntity(content, null);

            httpclient.execute(builder.build(), rsp -> rsp);
        }

        final Metadata meta = asyncOp().stat(path).join();
        assertEquals(content.length, meta.getContentLength());

        asyncOp().delete(path).join();
    }

    /**
     * Presign stat should succeed.
     */
    @Test
    public void testPresignStat() throws IOException {
        final String path = UUID.randomUUID().toString();
        final byte[] content = generateBytes();
        asyncOp().write(path, content).join();

        final PresignedRequest signedReq =
                asyncOp().presignStat(path, Duration.ofSeconds(3600)).join();
        try (CloseableHttpClient httpclient = HttpClients.createDefault()) {
            final ClassicRequestBuilder builder = createRequestBuilder(signedReq);

            final ClassicHttpResponse response = httpclient.execute(builder.build(), rsp -> rsp);
            assertEquals(HttpStatus.SC_OK, response.getCode());
            assertEquals(
                    String.valueOf(content.length),
                    response.getFirstHeader(HttpHeaders.CONTENT_LENGTH).getValue());
        }

        asyncOp().delete(path).join();
    }

    /**
     * Presign read should read content successfully.
     */
    @Test
    public void testPresignRead() throws IOException, NoSuchAlgorithmException {
        final String path = UUID.randomUUID().toString();
        final byte[] content = generateBytes();
        asyncOp().write(path, content).join();

        final PresignedRequest signedReq =
                asyncOp().presignRead(path, Duration.ofSeconds(3600)).join();
        try (CloseableHttpClient httpclient = HttpClients.createDefault()) {
            final ClassicRequestBuilder builder = createRequestBuilder(signedReq);

            final byte[] responseContent = httpclient.execute(builder.build(), rsp -> {
                return EntityUtils.toByteArray(rsp.getEntity());
            });
            assertEquals(content.length, responseContent.length);

            final MessageDigest digest = MessageDigest.getInstance("SHA-256");
            assertArrayEquals(digest.digest(content), digest.digest(responseContent));
        }

        asyncOp().delete(path).join();
    }

    private ClassicRequestBuilder createRequestBuilder(final PresignedRequest signedReq) {
        final ClassicRequestBuilder builder =
                ClassicRequestBuilder.create(signedReq.getMethod()).setUri(signedReq.getUri());
        for (Map.Entry<String, String> entry : signedReq.getHeaders().entrySet()) {
            // Skip content-length header, which is auto set by the http client.
            // If the header is set, the request will throw exception: Content-Length header already present.
            if (HttpHeaders.CONTENT_LENGTH.equalsIgnoreCase(entry.getKey())) {
                continue;
            }
            builder.addHeader(entry.getKey(), entry.getValue());
        }
        return builder;
    }
}
