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

import React, { useState } from "react";
import CodeWindow from "../ui/CodeWindow";
import styles from "./styles.module.css";

// The OpenDAL mental model in one picture, as the Rust core defines it: an
// application calls an operator, optional layers wrap the operator, and the
// operator talks to one configured service. Bindings map this model onto
// their own types — some rename or hide the operator, and not all expose
// layers; the page prose carries those caveats. Selecting a concept
// highlights its place in the diagram; the one-line hint lives in the window
// title bar. The diagram is decorative (aria-hidden) — the page prose carries
// the same information for assistive technology.
const CONCEPTS = [
  {
    id: "service",
    index: "01",
    name: "service",
    hint: "a backend described by plain config",
  },
  {
    id: "operator",
    index: "02",
    name: "operator",
    hint: "one handle for every storage call",
  },
  {
    id: "layer",
    index: "03",
    name: "layer",
    hint: "optional wrapper, sees every call",
  },
  {
    id: "operation",
    index: "04",
    name: "operation",
    hint: "the same verbs, bounded by capabilities",
  },
];

export default function MentalModel() {
  const [active, setActive] = useState("service");
  const current = CONCEPTS.find((concept) => concept.id === active);

  return (
    <CodeWindow
      as="figure"
      className={styles.model}
      data-active={active}
      title={
        <>
          {current.index} {current.name} — {current.hint}
        </>
      }
    >
      <div className={styles.diagram} aria-hidden="true">
        <div className={`${styles.gridLayer} odl-grid-bg`} />
        <div className={styles.backend}>
          <div className={styles.service}>
            <span className={styles.svcName}>
              <span className={styles.kind}>service · </span>s3
            </span>
            <code className={styles.code}>bucket = data</code>
          </div>
          <span className={styles.alts}>fs · gcs · azblob · 50+ more</span>
        </div>
        <span className={`${styles.flow} ${styles.flowBuild}`}>
          <span className={styles.flowLabel}>build</span>
        </span>
        <div className={styles.ringOuter}>
          <span className={styles.ringLabel}>
            <span className={styles.kind}>layer · </span>retry
          </span>
          <div className={styles.ringInner}>
            <span className={styles.ringLabel}>
              <span className={styles.kind}>layer · </span>logging
            </span>
            <div className={styles.core}>operator</div>
          </div>
        </div>
        <span className={`${styles.flow} ${styles.flowCall}`}>
          <span className={styles.flowLabel}>call</span>
        </span>
        <div className={styles.app}>
          <span className={styles.comment}>{"// your app"}</span>
          <code className={styles.code}>op.read("hello.txt")</code>
        </div>
      </div>
      <figcaption className={styles.legend}>
        {CONCEPTS.map((concept) => {
          const selected = active === concept.id;
          return (
            <button
              key={concept.id}
              type="button"
              className={`${styles.legendItem} ${
                selected ? styles.legendItemActive : ""
              }`}
              aria-pressed={selected}
              onMouseEnter={() => setActive(concept.id)}
              onFocus={() => setActive(concept.id)}
              onClick={() => setActive(concept.id)}
            >
              <span className={styles.legendIndex}>{concept.index}</span>
              <span className={styles.legendName}>{concept.name}</span>
            </button>
          );
        })}
      </figcaption>
    </CodeWindow>
  );
}
