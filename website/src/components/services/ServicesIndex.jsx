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

import React, { useMemo, useState } from "react";
import Layout from "@theme/Layout";
import Link from "@docusaurus/Link";
import styles from "./styles.module.css";

export default function ServicesIndex({ data }) {
  const { services } = data;
  const [query, setQuery] = useState("");

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    const list = q
      ? services.filter((s) => s.scheme.includes(q) || s.name.includes(q))
      : services;
    return [...list].sort((a, b) => a.scheme.localeCompare(b.scheme));
  }, [services, query]);

  return (
    <Layout
      title="Services"
      description="Browse every storage service OpenDAL supports, with a copy-paste configuration reference for each binding."
    >
      <div className={styles.page}>
        <header className={styles.header}>
          <h1 className={styles.title}>Services</h1>
          <p className={styles.subtitle}>
            Every storage service OpenDAL supports. Open one for its full
            configuration reference and ready-to-copy setup code in each binding.
          </p>
        </header>

        <input
          type="search"
          className={styles.search}
          placeholder={`Filter ${services.length} services…`}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          aria-label="Filter services"
        />

        {filtered.length === 0 ? (
          <p>No service matches “{query}”.</p>
        ) : (
          <ul className={styles.grid}>
            {filtered.map((s) => (
              <li key={s.scheme}>
                <Link to={`/services/${s.scheme}`} className={styles.card}>
                  <span className={styles.cardName}>{s.scheme}</span>
                  <span className={styles.cardMeta}>
                    {s.configCount} option{s.configCount === 1 ? "" : "s"}
                  </span>
                </Link>
              </li>
            ))}
          </ul>
        )}
      </div>
    </Layout>
  );
}
