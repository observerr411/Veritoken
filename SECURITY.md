# Security Policy

Veritoken handles real-world financial assets on-chain. Security vulnerabilities can result in direct financial loss to token holders. We take all reports seriously and ask that researchers follow this policy.

---

## Reporting a Vulnerability

**Do not open a public GitHub issue for security vulnerabilities.**

Please report security vulnerabilities through one of the following channels:

- **GitHub Private Advisory (preferred):** [Report a vulnerability](https://github.com/abore9769/Veritoken/security/advisories/new) — GitHub keeps the report private and notifies the maintainers immediately.
- **Email:** Send a detailed report to the repository maintainers. You can find contact information on the GitHub profiles of the repository owners.

Include as much of the following as possible in your report:

- A description of the vulnerability and the affected contract(s)
- The steps required to reproduce it, including any test case or proof-of-concept code
- The potential impact (e.g., fund loss, KYC bypass, unauthorized minting)
- The contract version or commit SHA where you observed the issue
- Any suggested mitigations you have identified

The more detail you provide, the faster we can triage and respond.

---

## Response Timeline

| Milestone | Target |
|---|---|
| Acknowledgment of receipt | Within 72 hours |
| Initial triage and severity assessment | Within 7 days |
| Status update to reporter | Every 14 days until resolved |
| Patch released and advisory published | Within 90 days of receipt |

If a vulnerability is particularly severe or complex, we may request an extension of the 90-day disclosure deadline. We will communicate this proactively and aim to resolve all issues as quickly as possible.

We will coordinate a public disclosure date with you and credit you in the advisory unless you prefer to remain anonymous.

---

## Supported Versions

Security fixes are applied to the latest version of the contracts on the `main` branch. Older deployments are not automatically patched — operators must redeploy updated contracts.

| Version | Supported |
|---|---|
| Latest (`main` branch) | Yes |
| Tagged releases | Only the most recent tag |
| Older tags / forks | No |

---

## Out of Scope

The following are **not** considered security vulnerabilities under this policy:

- Issues already publicly disclosed or present in open GitHub issues
- Theoretical vulnerabilities without a working proof of concept
- Findings from automated scanners with no demonstrated impact
- Vulnerabilities in third-party dependencies (report those upstream)
- Social engineering attacks
- Issues requiring physical access to the admin key or hardware wallet
- Frontend UI bugs that do not affect on-chain contract security

---

## Safe Harbor

We consider security research conducted under this policy to be authorised. We will not pursue legal action against researchers who:

- Discover and report vulnerabilities in good faith following this policy
- Do not exfiltrate, manipulate, or destroy data beyond what is necessary to demonstrate the vulnerability
- Do not exploit a vulnerability beyond the minimum necessary to confirm it exists
- Do not disclose the vulnerability publicly before the coordinated disclosure date
- Do not perform denial-of-service attacks against the network or any deployed contracts

We ask that you give us a reasonable amount of time to address the vulnerability before any public disclosure.
