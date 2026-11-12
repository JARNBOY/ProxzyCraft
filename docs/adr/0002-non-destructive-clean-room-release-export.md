---
status: accepted
---

# Non-Destructive Clean-Room Public Release Exporter

Rewriting history directly on active development branches with `git filter-repo` or force-pushing risks losing internal research logs, screenshots, or unmerged work-in-progress on collaborator clones. Instead, ProxzyCraft publishes public open-source releases via [`scripts/export-clean-release.sh`](file:///Users/paponsuapmongkonchai/MoneyWarProject/ProxzyCraft/scripts/export-clean-release.sh), leaving existing branches (`main`, feature branches, and backups) completely intact and never requiring `git push --force`.

By default (`--orphan`), the script archives `HEAD` into a fresh staging directory with a single MIT-licensed clean-room initial commit containing only tracked runtime source, documentation, and notices. A `--preserve-history` option uses `git filter-repo` (when installed) on a temporary clone rather than the active repository so git history scrubbing stays repeatable without risking data loss.

