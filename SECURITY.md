# Security policy

Security fixes target the latest release. Older versions may require an upgrade.

Report a suspected vulnerability using the repository's
[private vulnerability reporting form](https://github.com/lms21a/cfopsolver/security/advisories/new)
when available. If private reporting is unavailable, contact the repository owner
through their GitHub profile to arrange a private channel. Do not publish credentials,
private datasets, or exploit details in a public issue.

Include the affected version, platform, build features, minimal reproducer, and
impact. State-validation errors that could grant incorrect verifier rewards are
also useful reports. No response-time guarantee is made.

Inputs consume CPU and memory in proportion to their size and requested search
work. Applications evaluating untrusted model outputs should impose their own batch,
move-length, time, and memory limits. Probe limits bound two-phase search attempts;
they are not a wall-clock timeout for the whole CFOP pipeline.
