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

import React, { useEffect, useState } from "react";
import Layout from "@theme/Layout";
import CodeBlock from "@theme/CodeBlock";
import { useLocation } from "@docusaurus/router";
import Breadcrumb from "../ui/Breadcrumb";
import Tabs from "../ui/Tabs";
import styles from "./styles.module.css";

const LANGUAGE_ALIASES = {
  "c++": "cpp",
  cplusplus: "cpp",
  cpp: "cpp",
  cxx: "cpp",
};

function cx(...classNames) {
  return classNames.filter(Boolean).join(" ");
}

// Renders inline `code` spans inside an otherwise plain doc comment so the
// reference table stays readable without pulling in a full markdown renderer.
function renderComment(text) {
  if (!text) return null;
  const parts = text.split(/(`[^`]+`)/g);
  return parts.map((part, i) => {
    if (part.startsWith("`") && part.endsWith("`") && part.length > 1) {
      return <code key={i}>{part.slice(1, -1)}</code>;
    }
    return <span key={i}>{part}</span>;
  });
}

// One group's worth of options, rendered as a table. Default/example values
// (extracted from the config source) surface as chips next to the key.
function ConfigTable({ rows }) {
  return (
    <div className={styles.tableWrap}>
      <table className={styles.table}>
        <thead className={styles.tableHead}>
          <tr className={styles.tableHeaderRow}>
            <th className={styles.tableHeaderCell} scope="col">
              Key
            </th>
            <th className={styles.tableHeaderCell} scope="col">
              Type
            </th>
            <th className={styles.tableHeaderCell} scope="col">
              Required
            </th>
            <th className={styles.tableHeaderCell} scope="col">
              Description
            </th>
          </tr>
        </thead>
        <tbody className={styles.tableBody}>
          {rows.map((c) => {
            const rowClassName = cx(
              styles.tableRow,
              c.deprecated ? styles.deprecatedRow : ""
            );

            return (
              <tr key={c.name} className={rowClassName}>
                <td className={cx(styles.tableCell, styles.tableKeyCell)}>
                  <code className={styles.configKey}>{c.name}</code>
                  {c.deprecated && (
                    <span className={styles.badge}>deprecated</span>
                  )}
                  {c.default != null && (
                    <span className={styles.chip}>
                      default <code className={styles.chipCode}>{c.default}</code>
                    </span>
                  )}
                  {c.default == null && c.example != null && (
                    <span className={styles.chip}>
                      e.g. <code className={styles.chipCode}>{c.example}</code>
                    </span>
                  )}
                </td>
                <td className={styles.tableCell} data-label="Type">
                  {c.type}
                </td>
                <td className={styles.tableCell} data-label="Required">
                  {c.required ? "yes" : "no"}
                </td>
                <td
                  className={cx(styles.tableCell, styles.desc)}
                  data-label="Description"
                >
                  {renderComment(c.comments)}
                  {c.deprecated && c.deprecated.note && (
                    <div className={styles.deprecatedNote}>
                      {`Deprecated${
                        c.deprecated.since ? ` since ${c.deprecated.since}` : ""
                      }: `}
                      {renderComment(c.deprecated.note)}
                    </div>
                  )}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

// The deep-link routes (`/services/<scheme>/<binding>`) share this component
// with the bare service page; the trailing segment, when it names one of the
// available bindings, selects the initial tab.
function bindingFromPath(pathname, scheme, available) {
  const segments = pathname.split("/").filter(Boolean);
  const idx = segments.lastIndexOf(scheme);
  const tail = idx >= 0 ? segments[idx + 1] : undefined;
  return tail && available.includes(tail) ? tail : undefined;
}

function normalizeLanguage(value) {
  const normalized = value.trim().toLowerCase();
  return LANGUAGE_ALIASES[normalized] || normalized;
}

function safeDecodeURIComponent(value) {
  try {
    return decodeURIComponent(value);
  } catch {
    return undefined;
  }
}

// Keep raw `+` characters intact so links like `?language=c++` select C++.
// URLSearchParams decodes `+` as a space because it follows form encoding.
function languageFromSearch(search) {
  const query = search.startsWith("?") ? search.slice(1) : search;
  for (const part of query.split("&")) {
    const separator = part.indexOf("=");
    const rawKey = separator === -1 ? part : part.slice(0, separator);
    const rawValue = separator === -1 ? "" : part.slice(separator + 1);
    const key = safeDecodeURIComponent(rawKey);
    if (key !== "language") {
      continue;
    }

    return safeDecodeURIComponent(rawValue);
  }

  return undefined;
}

function bindingFromSearch(search, examples) {
  const language = languageFromSearch(search);
  if (!language) {
    return undefined;
  }

  const normalized = normalizeLanguage(language);
  return examples.find(
    (example) =>
      normalizeLanguage(example.binding) === normalized ||
      normalizeLanguage(example.language) === normalized
  )?.binding;
}

export default function ServicePage({ data }) {
  const { bindings, service } = data;
  const location = useLocation();

  const labelOf = Object.fromEntries(bindings.map((b) => [b.id, b.label]));
  const available = service.examples.map((e) => e.binding);
  const initial =
    bindingFromSearch(location.search, service.examples) ||
    bindingFromPath(location.pathname, service.scheme, available) ||
    available[0];

  const [active, setActive] = useState(initial);
  useEffect(() => {
    setActive(initial);
    // Re-sync when navigating between deep-link pages or URL-selected tabs.
  }, [location.pathname, location.search]);

  const example = service.examples.find((e) => e.binding === active);

  const title = `${service.scheme} | Services`;
  const description = `Configuration reference and copy-paste setup for the OpenDAL ${service.scheme} service.`;

  return (
    <Layout title={title} description={description}>
      <div className={styles.page}>
        <Breadcrumb
          aria-label="Breadcrumb"
          rootLabel="Services"
          rootHref="/services"
          items={[{ label: service.scheme }]}
        />

        <header className={styles.header}>
          <h1 className={styles.title}>{service.scheme}</h1>
          <p className={styles.subtitle}>
            {service.configs.length} configuration option
            {service.configs.length === 1 ? "" : "s"} · available in{" "}
            {available.map((id) => labelOf[id] || id).join(", ")}
          </p>
        </header>

        {example && (
          <section className={styles.section}>
            <Tabs
              items={service.examples}
              activeId={active}
              onChange={(e) => setActive(e.binding)}
              getId={(e) => e.binding}
              getLabel={(e) => labelOf[e.binding] || e.binding}
              aria-label="Choose a binding"
              controlsId="service-example-panel"
              id="service-example-tabs"
            />

            <div
              role="tabpanel"
              id="service-example-panel"
              aria-labelledby={`service-example-tabs-tab-${active}`}
            >
              <CodeBlock language={example.language} title="Quick start">
                {example.minimal}
              </CodeBlock>
            </div>

            <details className={styles.details}>
              <summary className={styles.detailsSummary}>
                All configuration options (copy &amp; trim)
              </summary>
              <CodeBlock language={example.language} title="Full reference">
                {example.full}
              </CodeBlock>
            </details>

            <p className={styles.hint}>
              Every option is passed as a string key; OpenDAL parses it into the
              right type. Some services may require building the binding with the
              matching <code>services-*</code> feature enabled.
            </p>
          </section>
        )}

        <section className={styles.section}>
          <h2>Configuration reference</h2>
          {service.configs.length === 0 ? (
            <p>This service takes no configuration options.</p>
          ) : (
            (service.groups.length ? service.groups : ["General"]).map(
              (group, i) => {
                const rows = service.configs.filter(
                  (c) => (c.group || "General") === group
                );
                if (rows.length === 0) return null;
                // First group expanded; later (advanced) groups collapsed so the
                // page opens focused on the essentials.
                return (
                  <details
                    key={group}
                    className={styles.group}
                    open={i === 0}
                  >
                    <summary className={styles.groupSummary}>
                      {group}
                      <span className={styles.groupCount}>{rows.length}</span>
                    </summary>
                    <ConfigTable rows={rows} />
                  </details>
                );
              }
            )
          )}
        </section>
      </div>
    </Layout>
  );
}
