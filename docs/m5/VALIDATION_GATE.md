# M5 — Validation Gate

Home Hub graduates from "project" to "product" only if real households keep
using it without help. M5 is a 4-week pilot with a hard go/no-go decision at
the end. This directory is everything needed to run it.

| File | Purpose |
|---|---|
| `VALIDATION_GATE.md` (this file) | The decision template — fill in, sign, archive |
| `PILOT_PLAYBOOK.md` | How to recruit, onboard, support, and interview pilot households |
| `metrics.sql` | Queries that turn each Hub's SQLite into pilot metrics |
| `NPS_SURVEY.md` | The exact survey script (one question + one follow-up) |

## The gate (from PRD §9 / IMPLEMENTATION_PLAN §M5)

| Metric | Target | Source |
|---|---|---|
| Pilot households | ≥ 20 households, 4 weeks each | pilot roster |
| Unaided photo backup | ≥ 60% of households complete a full first backup with **no support contact** | `metrics.sql` Q1 |
| Weekly retention | ≥ 70% of households transfer or back up something in week 4 | `metrics.sql` Q2 |
| Zero data loss | 0 confirmed cases of lost/corrupted user data | support log + `metrics.sql` Q3 (integrity events) |
| NPS | ≥ 40 | `NPS_SURVEY.md` |
| Idle resource use | < 80 MB RAM, < 1% CPU average on the hub laptop | `metrics.sql` Q4 + Task Manager spot checks |

## Decision rules

- **5+ of 6 targets met** → GO: proceed to polish/GA planning.
- **3–4 met, including zero data loss** → CONDITIONAL: one more 4-week pilot
  after fixing the top support theme.
- **Any confirmed data loss, or ≤ 2 targets met** → NO-GO: stop, do the
  retrospective, do not scale.

## Decision record (fill in at the end of the pilot)

```
Pilot window:        ____ → ____
Households enrolled: ____    Completed 4 weeks: ____
Unaided backup:      ____ %  (target ≥ 60)
Week-4 retention:    ____ %  (target ≥ 70)
Data-loss incidents: ____    (target 0)
NPS:                 ____    (target ≥ 40)
Idle RAM/CPU:        ____ MB / ____ %  (target < 80 MB, < 1%)

Top 3 support themes:
  1.
  2.
  3.

Decision:            GO / CONDITIONAL / NO-GO
Decided by:          ____    Date: ____
```
