# Skill Registry — binance-auditor1

> Index of skills available to this session. SDD skills (`sdd-*`), `_shared`, and
> `skill-registry` are intentionally excluded. Deduplicated by name;
> project-level skills take precedence over user-level skills.

Scanned paths:
- User skills: `~/.config/opencode/skills/`, `~/.agents/skills/`
- Project skills: `{project-root}/skills/`, `.opencode/skills/`, `.atl/skills/`, `.claude/skills/`, `.gemini/skills/`, `.cursor/skills/`, `.github/skills/`, `.codex/skills/`, `.qwen/skills/`, `.kiro/skills/`, `.openclaw/skills/`, `.pi/skills/`, `.agent/skills/`, `.agents/skills/`
- Result: no project-level skills found. All entries below are user-level.

## Convention Files

| File | Path | Notes |
|---|---|---|
| README | `README.md` | Boilerplate React+TS+Vite+Oxlint template notes |
| Install guide | `INSTALACION.md` | Spanish install/build/distribution guide for the desktop app |
| Global AGENTS | `~/.config/opencode/AGENTS.md` | Opencode agent persona + Engram protocol (not project-specific) |

## Available Skills

| Skill | Scope | Trigger | Path |
|---|---|---|---|
| branch-pr | user | creating, opening, or preparing PRs for review | `~/.config/opencode/skills/branch-pr/SKILL.md` |
| chained-pr | user | PRs over 400 lines, stacked PRs, review slices | `~/.config/opencode/skills/chained-pr/SKILL.md` |
| cognitive-doc-design | user | writing guides, READMEs, RFCs, onboarding, architecture, review-facing docs | `~/.config/opencode/skills/cognitive-doc-design/SKILL.md` |
| comment-writer | user | PR feedback, issue replies, reviews, Slack messages, GitHub comments | `~/.config/opencode/skills/comment-writer/SKILL.md` |
| customize-opencode | user (built-in) | editing opencode's own configuration (opencode.json/.opencode/~/.config/opencode) | `<built-in>` |
| find-skills | user | "how do I do X", "find a skill for X", discover/install skills | `~/.agents/skills/find-skills/SKILL.md` |
| go-testing | user | Go tests, go test coverage, Bubbletea teatest, golden files | `~/.config/opencode/skills/go-testing/SKILL.md` |
| issue-creation | user | creating GitHub issues, bug reports, feature requests | `~/.config/opencode/skills/issue-creation/SKILL.md` |
| judgment-day | user | judgment day, dual review, adversarial review, juzgar | `~/.config/opencode/skills/judgment-day/SKILL.md` |
| sanity-best-practices | user | Sanity schema design, GROQ, TypeGen, Visual Editing, Studio, framework integrations | `~/.agents/skills/sanity-best-practices/SKILL.md` |
| sanity-migration | user | Migrating CMSes into Sanity (AEM, Contentful, WordPress, etc.) | `~/.agents/skills/sanity-migration/SKILL.md` |
| skill-creator | user | new skills, agent instructions, documenting AI usage patterns | `~/.config/opencode/skills/skill-creator/SKILL.md` |
| skill-improver | user | improve/audit/refactor skills, skill quality | `~/.config/opencode/skills/skill-improver/SKILL.md` |
| work-unit-commits | user | plan commits as reviewable work units, commit splitting, chained PRs | `~/.config/opencode/skills/work-unit-commits/SKILL.md` |

## Relevant for This Project

- **cognitive-doc-design** — INSTALACION.md / README maintenance and any architecture docs (Spanish artifact language is the project convention).
- **work-unit-commits** / **branch-pr** / **chained-pr** — the repo has no CI gate yet; reviewable work units and PR hygiene matter most here.
- **issue-creation** — capturing audit/reconciliation bugs as GitHub issues.
- **go-testing** / **sanity-*** — NOT applicable (no Go, no Sanity).