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

import React, { useEffect, useId, useRef, useState } from "react";
import CodeBlock from "@theme/CodeBlock";
import IconCopy from "@theme/Icon/Copy";
import IconSuccess from "@theme/Icon/Success";
import CodeWindow, {
  CodeWindowBody,
  CodeWindowTitleLink,
} from "../ui/CodeWindow";
import IconAction from "../ui/IconAction";
import Tabs from "../ui/Tabs";
import styles from "./code-tabs.module.css";

const INLINE_LINK_PATTERN = /\[([^\]]+)\]\(([^)]+)\)/g;

function getInstallText(text) {
  return text.replace(INLINE_LINK_PATTERN, "$1");
}

function InstallTitle({ text }) {
  const links = [...text.matchAll(INLINE_LINK_PATTERN)];
  if (links.length === 0) return text;

  const title = [];
  let offset = 0;

  links.forEach((match) => {
    const [source, label, to] = match;
    const index = match.index ?? 0;

    if (index > offset) {
      title.push(text.slice(offset, index));
    }

    title.push(
      <CodeWindowTitleLink key={`${to}-${index}`} to={to}>
        {label}
      </CodeWindowTitleLink>,
    );
    offset = index + source.length;
  });

  if (offset < text.length) {
    title.push(text.slice(offset));
  }

  return <>{title}</>;
}

function copyText(text) {
  if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
    return navigator.clipboard.writeText(text);
  }

  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "");
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  document.body.appendChild(textarea);
  textarea.select();
  document.execCommand("copy");
  document.body.removeChild(textarea);
  return Promise.resolve();
}

function getInstallCopyText(text) {
  return getInstallText(text).replace(/^\$\s+/, "");
}

function InstallCopyButton({ text }) {
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    setCopied(false);
  }, [text]);

  useEffect(() => {
    if (!copied) return undefined;
    const timer = window.setTimeout(() => setCopied(false), 1000);
    return () => window.clearTimeout(timer);
  }, [copied]);

  async function onCopy() {
    try {
      await copyText(text);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  }

  return (
    <IconAction
      className={`${styles.installCopyButton} ${
        copied ? styles.installCopyButtonCopied : ""
      }`}
      onClick={onCopy}
      aria-label={copied ? "Copied" : `Copy ${text}`}
      title="Copy install command"
    >
      <span className={styles.installCopyIcons} aria-hidden="true">
        <IconCopy className={styles.installCopyIcon} />
        <IconSuccess className={styles.installCopySuccessIcon} />
      </span>
    </IconAction>
  );
}

export default function CodeTabs({ samples, title, equalize = false }) {
  const [active, setActive] = useState(0);
  const baseId = useId();
  const current = samples[active];
  const heading = current.install || title;

  const innerRef = useRef(null);
  const [bodyHeight, setBodyHeight] = useState(undefined);

  useEffect(() => {
    if (!equalize || !innerRef.current) return undefined;
    const measure = () => setBodyHeight(innerRef.current?.offsetHeight);
    measure();
    const ro =
      typeof ResizeObserver !== "undefined"
        ? new ResizeObserver(measure)
        : null;
    if (ro && innerRef.current) ro.observe(innerRef.current);
    return () => ro?.disconnect();
  }, [active, equalize]);

  return (
    <CodeWindow
      title={<InstallTitle text={heading} />}
      action={
        current.install ? (
          <InstallCopyButton text={getInstallCopyText(heading)} />
        ) : null
      }
    >
      <Tabs
        items={samples}
        activeId={current.id}
        onChange={(_, index) => setActive(index)}
        getId={(sample) => sample.id}
        getLabel={(sample) => sample.label}
        className={styles.codeTabs}
        aria-label="Choose a language"
        controlsId={`${baseId}-panel`}
        id={baseId}
      />

      <CodeWindowBody
        animated={equalize}
        style={
          equalize && bodyHeight != null ? { height: bodyHeight } : undefined
        }
        role="tabpanel"
        id={`${baseId}-panel`}
        aria-labelledby={`${baseId}-tab-${current.id}`}
      >
        <div ref={innerRef}>
          <CodeBlock language={current.language}>{current.code}</CodeBlock>
        </div>
      </CodeWindowBody>
    </CodeWindow>
  );
}
