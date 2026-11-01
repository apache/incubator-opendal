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

import React, { useId, useRef } from "react";
import styles from "./tabs.module.css";

export default function Tabs({
  items,
  activeId,
  onChange,
  getId = (item) => item.id,
  getLabel = (item) => item.label,
  controlsId,
  id,
  className = "",
  ...tabListProps
}) {
  const generatedId = useId();
  const baseId = id || generatedId;
  const tabRefs = useRef(new Map());
  const activeIndex = Math.max(
    0,
    items.findIndex((item) => getId(item) === activeId)
  );

  function selectByIndex(index, { focus = false } = {}) {
    const item = items[index];
    if (!item) {
      return;
    }

    onChange(item, index);

    if (focus) {
      tabRefs.current.get(getId(item))?.focus();
    }
  }

  function onKeyDown(event) {
    const keys = ["ArrowRight", "ArrowLeft", "Home", "End"];
    if (!keys.includes(event.key)) return;

    event.preventDefault();
    if (event.key === "Home") {
      selectByIndex(0, { focus: true });
      return;
    }
    if (event.key === "End") {
      selectByIndex(items.length - 1, { focus: true });
      return;
    }

    const dir = event.key === "ArrowRight" ? 1 : -1;
    selectByIndex((activeIndex + dir + items.length) % items.length, {
      focus: true,
    });
  }

  return (
    <div
      {...tabListProps}
      className={[styles.tabs, className].filter(Boolean).join(" ")}
      role="tablist"
      onKeyDown={onKeyDown}
    >
      {items.map((item, index) => {
        const itemId = getId(item);
        const selected = itemId === activeId;
        return (
          <button
            key={itemId}
            type="button"
            role="tab"
            id={`${baseId}-tab-${itemId}`}
            aria-selected={selected}
            aria-controls={controlsId || undefined}
            tabIndex={selected ? 0 : -1}
            className={`${styles.tab} ${selected ? styles.tabActive : ""}`}
            onClick={() => onChange(item, index)}
            ref={(node) => {
              if (node) {
                tabRefs.current.set(itemId, node);
              } else {
                tabRefs.current.delete(itemId);
              }
            }}
          >
            {getLabel(item)}
          </button>
        );
      })}
    </div>
  );
}
