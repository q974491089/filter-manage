# Filter Manage Design System

## Brand & Style

The design system is engineered for professional screen management, targeting creative professionals and power users who spend extended hours in front of displays. The personality is "Quietly Powerful" — it prioritizes the user's focus on the screen adjustments rather than the interface itself.

The aesthetic blends **Modern Corporate** precision with **Glassmorphism** depth. It leverages a sophisticated dark environment to reduce eye strain, using translucent layers and subtle blurs to create a sense of physical space. The emotional goal is to evoke a sense of precision, technical reliability, and visual comfort.

## Colors

Supports **dark mode** (default) and **light mode** via CSS custom properties. Toggle with the theme button in the header. Preference persists in localStorage.

### Core Palette

| Token | Dark | Light | Usage |
|-------|------|-------|-------|
| background | #111317 | #ffffff | Canvas |
| surface | #111317 | #ffffff | Canvas |
| surface-dim | #111317 | #e6e6eb | Dimmed surface |
| surface-bright | #37393e | #ffffff | Highlighted surface |
| surface-variant | #333539 | #e4e4e8 | Variant surface |
| surface-container-lowest | #0c0e12 | #ffffff | Deepest container |
| surface-container-low | #1a1c20 | #f5f5f8 | Low container |
| surface-container | #1e2024 | #f0f0f4 | Default container |
| surface-container-high | #282a2e | #ebebee | Elevated container |
| surface-container-highest | #333539 | #e4e4e8 | Highest container |
| on-background | #e2e2e8 | #1a1c1e | Text on background |
| on-surface | #e2e2e8 | #1a1c1e | Text on surface |
| on-surface-variant | #c2c6d6 | #44444e | Secondary text |
| outline | #8c909f | #767680 | Borders, dividers |
| outline-variant | #424754 | #c8c8d0 | Subtle borders |

### Primary

| Token | Dark | Light | Usage |
|-------|------|-------|-------|
| primary | #adc6ff | #3b82f6 | Primary actions, active states |
| on-primary | #002e6a | #ffffff | Text on primary |
| primary-container | #4d8eff | #d3e3fc | Primary containers |
| on-primary-container | #00285d | #002658 | Text on primary container |
| inverse-primary | #005ac2 | #adc6ff | Inverse primary |

### Secondary

| Token | Dark | Light | Usage |
|-------|------|-------|-------|
| secondary | #c0c1ff | #6366f1 | Secondary accent |
| on-secondary | #1000a9 | #ffffff | Text on secondary |
| secondary-container | #3131c0 | #e0e0ff | Secondary containers |
| on-secondary-container | #b0b2ff | #2020a0 | Text on secondary container |

### Tertiary

| Token | Dark | Light | Usage |
|-------|------|-------|-------|
| tertiary | #4edea3 | #10b981 | "Active" / "Safe" states |
| on-tertiary | #003824 | #ffffff | Text on tertiary |
| tertiary-container | #00a572 | #c5f4e0 | Tertiary containers |
| on-tertiary-container | #00311f | #003724 | Text on tertiary container |

### Error

| Token | Dark | Light | Usage |
|-------|------|-------|-------|
| error | #ffb4ab | #ba1a1a | Error states |
| on-error | #690005 | #ffffff | Text on error |
| error-container | #93000a | #ffdad6 | Error containers |
| on-error-container | #ffdad6 | #410002 | Text on error container |

### Accessibility

Dark mode: primary text at `White` / `Slate-200` against charcoal backgrounds (WCAG AAA).
Light mode: primary text at `#1a1c1e` against white/light-gray backgrounds (WCAG AAA).

## Typography

**Inter** for neutral, highly legible dark-environment text. **Geist** for technical metadata and labels ("developer-tool" aesthetic).

| Level | Font | Size | Weight | Line Height | Letter Spacing |
|-------|------|------|--------|-------------|----------------|
| headline-lg | Inter | 32px | 700 | 40px | -0.02em |
| headline-md | Inter | 24px | 600 | 32px | -0.01em |
| headline-sm | Inter | 18px | 600 | 24px | normal |
| body-lg | Inter | 16px | 400 | 24px | normal |
| body-md | Inter | 14px | 400 | 20px | normal |
| label-md | Geist | 12px | 500 | 16px | 0.05em |
| label-sm | Geist | 11px | 600 | 14px | 0.02em |

## Layout & Spacing

4px baseline grid.

| Token | Value | Usage |
|-------|-------|-------|
| unit | 4px | Base unit |
| xs | 4px | Minimal gaps |
| sm | 8px | Tight spacing |
| md | 16px | Compact bars, sidebars |
| lg | 24px | Cards, list items (airy feel) |
| xl | 40px | Section gaps |
| gutter | 20px | Column gutter |
| margin-mobile | 16px | Mobile margins |
| margin-desktop | 32px | Desktop margins |

### Grid

- **Desktop:** 12 columns, 1200px max-width
- **Tablet:** 8 columns (600px – 1024px)
- **Mobile:** 4 columns (< 600px)

## Elevation & Depth

Depth through **Tonal Layering** and **Glassmorphism**, not drop shadows.

**Dark mode:**
1. Base: `#111317` — the canvas.
2. Middle (Cards/Panels): `#1a1c20` with 1px border `rgba(255, 255, 255, 0.08)`.
3. Top (Modals/Popovers): `backdrop-filter: blur(20px)` with `rgba(30, 41, 59, 0.7)`.

**Light mode:**
1. Base: `#ffffff` — the canvas.
2. Middle (Cards/Panels): `#f0f0f4` with 1px border `rgba(0, 0, 0, 0.08)`.
3. Top (Modals/Popovers): `backdrop-filter: blur(20px)` with `rgba(255, 255, 255, 0.8)`.

Inner glows (1px stroke inside top edge) on primary buttons for a "lit from within" appearance (dark mode). In light mode, use subtle shadow instead.

## Shapes (Border Radius)

| Token | Value | Usage |
|-------|-------|-------|
| sm | 0.25rem | Minimal rounding |
| DEFAULT | 0.5rem | Buttons, inputs |
| md | 0.75rem | Intermediate |
| lg | 1rem | Cards, containers |
| xl | 1.5rem | Large containers |
| full | 9999px | Pills, circles |

## Components

### Buttons

- **Primary:** Solid primary color, on-primary text. 10% brightness increase on hover.
- **Secondary:** Transparent, 1px border with outline-variant at 30% opacity.
- **Tertiary/Ghost:** No background/border, text only, on-surface on hover.

### Inputs & Sliders

- **Sliders:** Track `#2D3139`, active fill Primary Blue, thumb high-contrast white circle.
- **Inputs:** Darker than surface for "inset" feel. Focus: 2px Primary Blue glow.

### Cards

- 16px padding. Titles = Headline-SM. Subtle 1px top border highlight.

### Lists

- 12px vertical padding per item. Separator lines only if needed; otherwise alternating background tints.

### Progress Indicators

- Primary color for active progress. "pulse" animation for active background filter processing.
