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

import React from "react";
import Link from "@docusaurus/Link";
import styles from "./code-window.module.css";

function CodeWindowDots() {
  return (
    <div className={styles.windowDots} aria-hidden="true">
      <span className={styles.windowDot} />
      <span className={styles.windowDot} />
      <span className={styles.windowDot} />
    </div>
  );
}

export function CodeWindowTitleLink({ className = "", ...props }) {
  return (
    <Link
      {...props}
      className={[styles.windowTitleLink, className].filter(Boolean).join(" ")}
    />
  );
}

export function CodeWindowBar({ title, action }) {
  return (
    <div className={styles.windowBar}>
      <CodeWindowDots />
      <span className={styles.windowTitle}>{title}</span>
      {action}
    </div>
  );
}

export function CodeWindowBody({
  animated = false,
  className = "",
  children,
  ...props
}) {
  const classes = [
    styles.codeBody,
    animated ? styles.codeBodyAnimated : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div {...props} className={classes}>
      {children}
    </div>
  );
}

export default function CodeWindow({
  as: Component = "div",
  title,
  action,
  className = "",
  children,
  ...props
}) {
  return (
    <Component
      {...props}
      className={[styles.codeWindow, className].filter(Boolean).join(" ")}
    >
      <CodeWindowBar title={title} action={action} />
      {children}
    </Component>
  );
}
