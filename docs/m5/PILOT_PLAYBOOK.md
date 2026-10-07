# Pilot Playbook (M5)

Four weeks, ≥ 20 households, real laptops, real photos. The point is to learn
whether Home Hub survives contact with reality — not to make it look good.

## 1. Recruiting

- Target: households with an old Windows laptop they no longer use and at
  least one Android phone full of photos.
- Mix: ~half "comfortable with technology", half not. All from your extended
  network is fine; all from a developer Slack is not.
- Screen out: laptops older than ~2013, single-core, < 4 GB RAM, or machines
  the family still depends on daily (check against `compat/compat-db.yaml`).
- Get explicit consent: their photos live on their own hardware, you will
  never see file contents, and they can leave any time.

## 2. Onboarding (target: < 15 minutes, unaided after the install)

1. Installer on the old laptop (`installer/` → Inno Setup or WiX build).
2. Hub dashboard opens automatically → "Add a device" QR on screen.
3. Household installs the Android APK (or TestFlight build later) and scans.
4. Point them at the share sheet once: "share any photo → Home Hub".
5. Then leave. Everything after this must work without you.

## 3. During the pilot

- One support channel (a group chat is fine). Log every contact in the
  support log: date, household, theme, resolution time.
- Do NOT remotely fix things for them unless they ask; "unaided" is the metric.
- Check hub dashboards only when invited (screenshots of Overview are fine).
- Watch for silent churn: a household that stops using it counts; interview
  them at week 4 even if they ghost you.

## 4. Week-4 interview (15 minutes per household)

1. Show me the last thing you sent Home. (Watch, don't tell.)
2. Have your photos finished backing up? How do you know?
3. Did anything ever feel lost or confusing? What?
4. What would you miss if we turned this off tomorrow?
5. The NPS question (see `NPS_SURVEY.md`) — asked exactly as written.

## 5. Metrics collection

Each household (or you, during the interview) runs `metrics.sql` against
`%ProgramData%\HomeHub\hub.db` with `sqlite3 -readonly`, and pastes output.
No telemetry leaves the household by default (SECURITY §8) — collection is
manual and consented, every time.

## 6. After the gate

- Fill in `VALIDATION_GATE.md`, sign, archive with the pilot data.
- Whatever the decision: send every household a thank-you and a one-paragraph
  summary of what you learned. They gave you four weeks; give them closure.
