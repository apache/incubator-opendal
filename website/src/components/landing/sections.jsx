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
import Link from "@docusaurus/Link";
import { useBaseUrlUtils } from "@docusaurus/useBaseUrl";
import CodeTabs from "./CodeTabs";
import ExplorerSection from "./ExplorerSection";
import styles from "./styles.module.css";
import {
  REPO_URL,
  DOCS_URL,
  DISCORD_URL,
  heroStats,
  codeSamples,
  valueProps,
  capabilityThemes,
  usedBy,
  USERS_LIST_URL,
  serviceGroups,
  bindings,
  layers,
} from "./data";

export function Hero() {
  return (
    <header className={styles.hero}>
      <div className={`${styles.heroGrid} odl-grid-bg`} aria-hidden="true" />
      <div className="odl-container">
        <div className={styles.heroInner}>
          <div>
            <span className="odl-eyebrow">
              Apache OpenDAL™ — Open Data Access Layer
            </span>
            <h1 className={styles.heroTitle}>
              One Layer,
              <br />
              <span className={styles.heroTitleAccent}>All Storage.</span>
            </h1>
            <p className={styles.heroLede}>
              One zero-cost{" "}
              <span className={styles.termItalic}>operator</span> for object
              storage, file systems, databases and more — in the language you
              already ship.
            </p>
            <div className={styles.heroActions}>
              <Link
                className={`${styles.btn} ${styles.btnPrimary}`}
                to={DOCS_URL}
              >
                Get started <span className={styles.btnArrow}>→</span>
              </Link>
              <Link
                className={`${styles.btn} ${styles.btnSecondary}`}
                to={REPO_URL}
              >
                View on GitHub
              </Link>
            </div>
            <div className={styles.heroStats}>
              {heroStats.map((stat) => (
                <div className={styles.heroStat} key={stat.label}>
                  <span className={styles.heroStatValue}>{stat.value}</span>
                  <span className={styles.heroStatLabel}>{stat.label}</span>
                </div>
              ))}
            </div>
          </div>
          <div className={styles.heroAside}>
            <CodeTabs samples={codeSamples} title="quickstart" equalize />
          </div>
        </div>
      </div>
    </header>
  );
}

export function UsedBy() {
  const { withBaseUrl } = useBaseUrlUtils();
  return (
    <section className={`${styles.section} ${styles.sectionSubtle}`}>
      <div className="odl-container">
        <div className={styles.sectionHead}>
          <span className="odl-eyebrow">Used by</span>
          <h2 className={styles.sectionTitle}>
            Powering AI, analytics, and real-time data
          </h2>
          <p className={styles.sectionLede}>
            OpenDAL runs in production across the open-source ecosystem. These
            are some of the projects that build on it.
          </p>
        </div>
        <ul className={styles.logoWall}>
          {usedBy.map((u) => (
            <li className={styles.logoWallItem} key={u.name}>
              <Link className={styles.logoItem} to={u.href} title={u.name}>
                <img
                  className={styles.logoMark}
                  src={withBaseUrl(u.icon)}
                  alt=""
                  width="28"
                  height="28"
                  loading="lazy"
                />
                <span className={styles.logoName}>{u.name}</span>
              </Link>
            </li>
          ))}
          <li className={styles.logoWallItem}>
            <Link className={styles.actionLogo} to={USERS_LIST_URL}>
              + Add your logo
            </Link>
          </li>
        </ul>
      </div>
    </section>
  );
}

export function ValueProps() {
  return (
    <section className={styles.section}>
      <div className="odl-container">
        <div className={styles.sectionHead}>
          <span className="odl-eyebrow">Why OpenDAL</span>
          <h2 className={styles.sectionTitle}>
            A storage layer you can build production on.
          </h2>
          <p className={styles.sectionLede}>
            OpenDAL turns one vision — <em>One Layer, All Storage</em> — into a
            practical foundation for applications, libraries and data systems.
          </p>
        </div>
        <div className={`${styles.valueGrid} ${styles.reveal}`}>
          {valueProps.map((v) => (
            <article className={styles.valueCard} key={v.index}>
              <span className={styles.valueIndex}>{v.index}</span>
              <h3 className={styles.valueCardTitle}>{v.title}</h3>
              <p className={styles.valueCardBody}>{v.body}</p>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

export function Capabilities() {
  const [active, setActive] = useState(capabilityThemes[0]);
  return (
    <section className={`${styles.section} ${styles.sectionSubtle}`}>
      <div className="odl-container">
        <div className={styles.sectionHead}>
          <span className="odl-eyebrow">Capabilities</span>
          <h2 className={styles.sectionTitle}>Configure once. Access anything.</h2>
          <p className={styles.sectionLede}>
            One <span className={styles.termItalic}>operator</span> is a full
            toolkit for real-world data — read and write at scale, recover from
            failures, and work with files — the same way on every backend.
          </p>
        </div>
        <ExplorerSection
          id="landing-capability-preview"
          items={capabilityThemes}
          activeItem={active}
          onSelect={setActive}
          getKey={(theme) => theme.title}
          getTitle={(theme) => theme.title}
          getDescription={(theme) => theme.blurb}
          getDoc={(theme) => theme.doc}
          getCode={(theme) => theme.code}
        />
      </div>
    </section>
  );
}

export function Services() {
  const { withBaseUrl } = useBaseUrlUtils();
  return (
    <section className={styles.section}>
      <div className="odl-container">
        <div className={styles.sectionHead}>
          <span className="odl-eyebrow">Services</span>
          <h2 className={styles.sectionTitle}>
            50+ storage services, one interface.
          </h2>
          <p className={styles.sectionLede}>
            Enable only the backends your application needs. The{" "}
            <span className={styles.termItalic}>Operator</span> contract stays
            identical across every one.
          </p>
        </div>
        <div className={`${styles.serviceGroups} ${styles.reveal}`}>
          {serviceGroups.map((group) => (
            <div className={styles.serviceGroup} key={group.category}>
              <h3 className={styles.serviceGroupTitle}>{group.category}</h3>
              <div className={styles.serviceChips}>
                {group.services.map((s) => (
                  // Service docs live at /services/<scheme>, registered by
                  // plugins/services-docs-plugin.js; s.name is the scheme.
                  <Link
                    className={styles.serviceChip}
                    to={`/services/${s.name}`}
                    key={s.name}
                  >
                    <img
                      className={styles.serviceChipIcon}
                      src={withBaseUrl(s.icon)}
                      alt=""
                      width="15"
                      height="15"
                      loading="lazy"
                    />
                    {s.name}
                  </Link>
                ))}
              </div>
            </div>
          ))}
        </div>
        <div className={styles.servicesFoot}>
          <Link
            className={`${styles.btn} ${styles.btnSecondary}`}
            to="/services/"
          >
            Browse all services <span className={styles.btnArrow}>→</span>
          </Link>
        </div>
      </div>
    </section>
  );
}

export function Bindings() {
  const { withBaseUrl } = useBaseUrlUtils();
  return (
    <section className={`${styles.section} ${styles.sectionSubtle}`}>
      <div className="odl-container">
        <div className={styles.sectionHead}>
          <span className="odl-eyebrow">Bindings</span>
          <h2 className={styles.sectionTitle}>Ship it in your language.</h2>
          <p className={styles.sectionLede}>
            Each binding exposes the same OpenDAL service model while following
            its own ecosystem's conventions.
          </p>
        </div>
        <div className={`${styles.bindingGrid} ${styles.reveal}`}>
          {bindings.map((b) => (
            <Link className={styles.bindingCard} to={b.doc} key={b.name}>
              <img
                className={styles.bindingIcon}
                src={withBaseUrl(b.icon)}
                alt=""
                width="30"
                height="30"
                loading="lazy"
              />
              <span className={styles.bindingName}>{b.name}</span>
            </Link>
          ))}
        </div>
      </div>
    </section>
  );
}

export function Layers() {
  const [active, setActive] = useState(layers[0]);
  return (
    <section className={styles.section}>
      <div className="odl-container">
        <div className={styles.sectionHead}>
          <span className="odl-eyebrow">Layers</span>
          <h2 className={styles.sectionTitle}>
            Production behavior, composed — not coded.
          </h2>
          <p className={styles.sectionLede}>
            Stack cross-cutting concerns as reusable layers. The order is
            explicit and the core stays zero-cost.
          </p>
        </div>
        <ExplorerSection
          id="landing-layer-preview"
          items={layers}
          activeItem={active}
          onSelect={setActive}
          getKey={(layer) => layer.name}
          getTitle={(layer) => layer.name}
          getDescription={(layer) => layer.desc}
          getDoc={(layer) => layer.doc}
          getCode={(layer) => layer.code}
          variant="tiles"
        />
      </div>
    </section>
  );
}

export function FinalCta() {
  return (
    <section className={`${styles.section} ${styles.sectionSubtle}`}>
      <div className="odl-container">
        <div className={styles.finalCta}>
          <div className={`${styles.finalCtaInner} ${styles.finalCenter}`}>
            <span className={`odl-eyebrow ${styles.finalEyebrow}`}>
              Start building
            </span>
            <h2 className={styles.finalCtaTitle}>
              One layer for all your storage.
            </h2>
            <p className={styles.finalCtaLede}>
              Join the infrastructure builders, platform teams and application
              developers accessing data freely, painlessly and efficiently.
            </p>
            <div className={styles.finalCtaActions}>
              <Link
                className={`${styles.btn} ${styles.btnPrimary}`}
                to={DOCS_URL}
              >
                Get started <span className={styles.btnArrow}>→</span>
              </Link>
              <Link
                className={`${styles.btn} ${styles.btnSecondary}`}
                to={REPO_URL}
              >
                Star on GitHub
              </Link>
              <Link
                className={`${styles.btn} ${styles.btnSecondary}`}
                to={DISCORD_URL}
              >
                Join Discord
              </Link>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
