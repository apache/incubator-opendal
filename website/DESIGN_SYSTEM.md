<!--
 Licensed to the Apache Software Foundation (ASF) under one
 or more contributor license agreements.  See the NOTICE file
 distributed with this work for additional information
 regarding copyright ownership.  The ASF licenses this file
 to you under the Apache License, Version 2.0 (the
 "License"); you may not use this file except in compliance
 with the License.  You may obtain a copy of the License at

   http://www.apache.org/licenses/LICENSE-2.0

 Unless required by applicable law or agreed to in writing,
 software distributed under the License is distributed on an
 "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
 KIND, either express or implied.  See the License for the
 specific language governing permissions and limitations
 under the License.
-->

# OpenDAL Design System

> **One Layer, All Storage.** A precise, engineering-grade visual language for
> Apache OpenDAL™ — built to feel as dependable, fast and composable as the
> library itself.

This document is the source of truth for OpenDAL's web visual language. The
landing page (`src/pages/index.jsx`) is its first full application; docs and
blog inherit it through Infima variable mapping.

---

## 1. Design principles

The visual system is a direct translation of OpenDAL's engineering values.

| Library value             | Visual expression                                                            |
| ------------------------- | ---------------------------------------------------------------------------- |
| **One Layer, All Storage**| One accent, one type voice, one grid — convergence over decoration.          |
| **Solid Foundation**      | Hairline structure, precise alignment, restraint. Nothing is decorative-only.|
| **Fast Access (zero-cost)**| System fonts, no web-font requests, CSS-only motion. The page costs nothing it doesn't need. |
| **Object Storage First**  | Content-first layout; code and capabilities lead, ornament follows.          |
| **Extensible Architecture**| CSS variables compose; components are presentational and reusable.          |

Operating posture: **engineering-minimal, light-first** (dark mode is a
first-class, fully designed peer), near-monochrome **ink** with a single
**precise-blue** accent, and the signature solid **bar** lifted from the
OpenDAL wordmark.

---

## 2. Where the system lives

| Concern                         | Location                                  |
| ------------------------------- | ----------------------------------------- |
| CSS variables + Infima mapping | `src/css/variables.css`                     |
| Global styles + motif          | `src/css/global.css`                        |
| Landing sections                | `src/components/landing/sections.jsx`     |
| Code window / language tabs     | `src/components/landing/CodeTabs.jsx`     |
| Landing styles                  | `src/components/landing/styles.module.css`|
| Content model (copy, catalogs)  | `src/components/landing/data.js`          |

All shared design values are encoded as CSS custom properties, also called CSS
variables, prefixed `--odl-*` in `src/css/variables.css`. Components never
hardcode raw values when a matching `--odl-*` variable exists.

---

## 3. Color

### 3.1 Primitives

**Ink** — a cool, neutral near-black → white ramp (the monochrome backbone):

`--odl-ink-{0,50,100,150,200,300,400,500,600,700,800,850,900,950}`
from `#ffffff` to `#0a0d11`.

**Precise blue** — the single brand accent:

`--odl-blue-{50…900}` from `#edf3ff` to `#163172`; `--odl-blue-600 #1e54e0` is
the canonical light-mode accent.

Functional signals (`--odl-green/amber/red-*`) exist mostly so inherited docs
UI (admonitions, etc.) stay on-system. They are intentionally muted.

### 3.2 Semantic roles (theme-aware)

Components use **semantic** CSS variables, not primitive color variables:

| CSS variable          | Light                | Dark              | Use                          |
| --------------------- | -------------------- | ----------------- | ---------------------------- |
| `--odl-bg`            | `ink-0`              | `#0a0d11`         | Page background              |
| `--odl-bg-subtle`     | `ink-50`             | `#0e1218`         | Alternating section bands    |
| `--odl-surface`       | `ink-0`              | `#11161c`         | Cards, code window           |
| `--odl-fg`            | `ink-900`            | `#e7ebf0`         | Body text                    |
| `--odl-fg-muted`      | `ink-600`            | `#9aa6b3`         | Secondary text               |
| `--odl-border`        | `ink-200`            | `#242c35`         | Hairlines, dividers          |
| `--odl-bar`           | `ink-950`            | `#f6f8fa`         | The bar motif                |
| `--odl-accent`        | `blue-600`           | `blue-300`        | Links, emphasis              |
| `--odl-action-bg`     | `blue-600`           | `blue-500`        | Solid CTA background         |

### 3.3 Rules

- **Black/white leads, blue accents.** Blue is for interaction, emphasis and
  code highlight — never large fills (except the solid CTA).
- **Contrast:** body and accent text meet WCAG AA (≥ 4.5:1) in both themes;
  white-on-`action-bg` passes for buttons. Dark mode lifts the accent to a
  lighter blue rather than inverting it.
- **Never rely on color alone** — pair with the bar, an icon, or a label.

---

## 4. Typography

### 4.1 Stacks & rationale

```
--odl-font-sans: system-ui, -apple-system, "Segoe UI", Roboto, …
--odl-font-mono: ui-monospace, "SF Mono", "Cascadia Code", "JetBrains Mono", …
```

System stacks are a **deliberate** choice, not a fallback: zero third-party
requests (ASF privacy-friendly), zero layout shift, instant and globally
consistent rendering (OpenDAL has a large global user base). The wordmark
already carries the brand letterforms.

**Monospace is promoted to a brand voice.** It marks anything structural or
technical — eyebrows/kickers, section indices, statistics, service names, code.
This gives the minimal system its engineering character without a custom font.

> To adopt **IBM Plex** instead: self-host via `@fontsource/ibm-plex-sans` and
> `@fontsource/ibm-plex-mono`, `@import` the weights in `variables.css`, and set
> `--odl-font-sans`/`--odl-font-mono` accordingly. The rest of the system needs
> no changes.

### 4.2 Scale

`--odl-text-2xs (11px)` · `xs (12)` · `sm (14)` · `base (16)` · `md (18)` ·
`lg (22)` · `xl (28)` · `2xl (36)` · `3xl (48)` · `display clamp(44–80px)`.

Headings: weight `650–700`, tracking `-0.022em`. Body: `400`, line-height
`1.6`. Mono labels: uppercase, tracking `0.12em`.

---

## 5. Spacing & layout

- **4px rhythm:** `--odl-space-1…10` (`0.25rem` → `8rem`).
- **Container:** `--odl-container 1200px`, `--odl-container-narrow 880px`,
  fluid `--odl-gutter clamp(1.25–2.5rem)`. Use the global `.odl-container`.
- **Section rhythm:** vertical padding `clamp(3.5rem, 7rem)`, every section
  separated by a single hairline (`border-top: 1px var(--odl-border)`),
  alternating with `--odl-bg-subtle` bands.

---

## 6. Radius & elevation

- **Radius:** `--odl-radius-xs 3 / sm 5 / md 8 / lg 12 / pill`. Restrained by
  design. **The bar motif is always sharp (radius 0).**
- **Elevation:** `--odl-shadow-sm/md/lg`. Prefer **border-based** separation;
  reserve real shadow for floating surfaces (the code window).

---

## 7. Motion

- Use `--odl-dur-fast 120ms`, `--odl-dur 200ms`, and
  `--odl-dur-slow 320ms` with `--odl-ease` or `--odl-ease-out`.
- Keep micro-interactions short: hover, tab changes, and button presses should
  finish within 120–200ms.
- Use CSS-only reveal-on-scroll through `.reveal` and `animation-timeline:
  view()`. Browsers without support must show content immediately.
- Animate `opacity` and `transform` by default. Animate layout only when it
  fixes a real layout jump, keep the movement local, and disable it for
  `prefers-reduced-motion`.
- Never block input with animation.

---

## 8. The bar motif

The solid rectangle from the OpenDAL wordmark is the system's signature mark.

- `.odl-bar` — a sharp solid rectangle for dividers/emphasis.
- `.odl-eyebrow` — the canonical section kicker: monospace, uppercase, led by a
  small **accent bar**. Used above every section title.
- As a top-border accent on principle cards, and as the section-title marker.

Used consistently, the bar makes the minimal system unmistakably OpenDAL.

---

## 9. Component patterns

| Component        | Class(es)                                  | Notes                                            |
| ---------------- | ------------------------------------------ | ------------------------------------------------ |
| Primary button   | `.btn .btnPrimary`                         | Solid `action-bg`; one primary action per view.  |
| Secondary button | `.btn .btnSecondary`                       | Hairline outline, ink text.                      |
| On-dark buttons  | `.btnOnDark` / `.btnOnDarkGhost`           | For the dark CTA panel.                           |
| Eyebrow          | `.odl-eyebrow` (global)                    | Mono + accent bar; section kicker.               |
| Code window      | `CodeTabs`                                 | Window chrome + language tabs + Prism highlight. |
| Value card       | `.valueCard`                               | Mono index, title, body; 1px-gap grid.           |
| Service chip     | `.serviceChip`                             | Mono name + 15px icon, grouped by family.        |
| Binding / layer  | `.bindingCard` / `.layerItem`              | Icon + mono label in a hairline grid.            |

Layout grids use a **1px gap over a border-colored background** to render crisp
hairline dividers between cells — a precise, technical signature.

---

## 10. Do / Don't

**Do**
- Lead with ink; use blue for interaction and emphasis.
- Use monospace for labels, indices, figures and code.
- Keep one primary CTA per view; separate secondary actions.
- Reach for hairlines and spacing before shadows and fills.
- Design and test light **and** dark together.

**Don't**
- Introduce a second accent hue or gradients-as-decoration.
- Use emoji as icons (use the existing SVG sets in `static/img/*`).
- Hardcode hex in components when a matching `--odl-*` CSS variable exists.
- Add motion that conveys nothing or ignores `prefers-reduced-motion`.

---

## 11. Accessibility checklist

- [ ] Text/accent contrast ≥ 4.5:1 (AA) in light and dark.
- [ ] Show a visible `:focus-visible` state on every interactive element. Use
      the 2px accent ring by default. Use a compact accent outline, underline,
      or border for dense icon controls, window chrome links, and text inputs
      when the default ring overpowers the control.
- [ ] Decorative icons use empty `alt`; meaning carried by adjacent text.
- [ ] Code tabs are a proper `tablist`/`tab`/`tabpanel` with arrow-key support.
- [ ] `prefers-reduced-motion` respected globally.
- [ ] Layout holds at 375 / 768 / 1024 / 1440px without horizontal scroll.
