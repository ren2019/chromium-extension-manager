# Spec #10 / ticket #11 — native browsing and detail

2026-10-30. Implemented against the approved HTML commit `959eefcc89fd5f72bead89138ab656a7eda8aaae`, in the existing Rust/GPUI application.

## Scope delivered

Light component theme; one titlebar with application logo/full name; embedded browser and Lucide icons; flat floating profile selector with managed counts and pins; current-profile library/attention/history; full-page detail with centralized existing actions; initially folded cross-profile comparison; About page. Existing source/install/update use cases remain available for subsequent tickets to refine.

Local manifest descriptions and decoded artwork use a read-only public Manager boundary. Reject escaping paths and external symlinks; invalid artwork does not block browsing. Related installations use matching local paths, repository/subdirectory, or exact Release repository/asset name/root. The existing release selector strips version digits, so it is deliberately **not** evidence for cross-version grouping; ambiguous releases remain separate.

Dates display local time from existing records. Initial deployment uses one timestamp for installed/updated; browsing, checks and confirmation do not rewrite those timestamps. Unrecorded old values remain unrecorded.

## Validation

- Public use-case TDD: metadata description and unsafe path fallback first failed due to missing API, then passed; a shared guessed Release selector first incorrectly merged distinct assets, then exact-artifact matching passed.
- Four presentation tests cover real local PNG decoding, corruption and symlink fallback, same-name different sources, unrelated release artifacts, read-only related-install navigation data, and reopen persistence.
- `cargo test --workspace`: 50 tests passed. Native build and Clippy with warnings denied passed (dependency `block` retains Cargo's existing future-incompatibility advisory).
- Parent agent used the actual native app with isolated data at 1120×780. Verified library, full detail, collapsed comparison/navigation, profile counts, two pins, remembered selection and empty state. Review fixes included readable AX names, profile truncation/tooltip, equal-width detail cards, blue selection styling, and a single light titlebar. Screenshot evidence collected by parent under `/tmp/cepm-implementation-10/screenshots/`; final packaged acceptance belongs to the integration work.
- No personal browser writes or extension uninstall took place in this ticket. No Windows runtime verification claimed. The subsequent install/confirmation, repair and Edge tickets are not represented as complete by this visual work.

## Keyboard follow-up

The native smoke exposed that component buttons had built-in tab stops but the application had not created a tab group or an initial focus path. The application now establishes GPUI's tab group, uses the existing button keyboard/ring handling, and uses GPUI's focus traps for the Profile menu and task dialog. Escape closes the overlay; focus returns to its surviving keyboard trigger, otherwise the Profile selector. Navigation that removes the focused row re-establishes the root focus path.

Parent native validation: initial Tab/Return opens Profile; Tab visibly moves the focus ring inside the menu; Escape returns to the trigger and Return reopens it. Screenshot: `/tmp/cepm-implementation-10/screenshots/11-keyboard-menu.png`. The native accessibility tool continued reporting the window as focused, so behavior and visible ring—not that tool's focus field—were used as evidence. Focus accent uses the approved blue (`#3b82f6`).
