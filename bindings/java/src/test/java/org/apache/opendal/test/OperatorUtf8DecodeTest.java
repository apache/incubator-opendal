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
import java.nio.file.Path;
import org.apache.opendal.Metadata;
import org.apache.opendal.Operator;
import org.apache.opendal.ServiceConfig;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

public class OperatorUtf8DecodeTest {
    @TempDir
    private static Path tempDir;

    /**
     * Write file with non ascii name should succeed.
     *
     * @see <a href="https://github.com/apache/opendal/issues/3194">More information</a>
     */
    @Test
    public void testWriteFileWithNonAsciiName() {
        final ServiceConfig.Fs fs =
                ServiceConfig.Fs.builder().root(tempDir.toString()).build();

        try (final Operator op = Operator.of(fs)) {
            final String path = "❌😱中文.test";
            final byte[] content = "❌😱中文".getBytes();
            op.write(path, content);
            final Metadata meta = op.stat(path);
            assertThat(meta.isFile()).isTrue();
            assertThat(meta.getContentLength()).isEqualTo(content.length);

            op.delete(path);
        }
    }
}
