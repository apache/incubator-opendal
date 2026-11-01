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

package org.apache.opendal.layer;

import org.apache.opendal.Layer;

/**
 * Layer that overrides the full capability exposed by an operator.
 */
public class CapabilityOverrideLayer extends Layer {

    private final String overrides;

    public CapabilityOverrideLayer(String overrides) {
        this.overrides = overrides;
    }

    @Override
    protected long layer(long nativeOp) {
        return doLayer(nativeOp, overrides);
    }

    private static native long doLayer(long nativeHandle, String overrides);
}
