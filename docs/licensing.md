# Licensing

Pageframe's own code is MIT OR Apache-2.0. You can use it in a closed-source or commercial app. Stylo, the CSS engine, and several dependencies use MPL-2.0. You can include them if you make their source available.

This is a summary of the licenses, not legal advice. Patent and codec rights are a separate question.

The crate counts use the 24 November 2026 `Cargo.lock` with every feature enabled and no target filter.

## How the check runs

`.github/lint/repo_checks.sh` runs cargo-deny in the `checks` job in CI and in the `pre-push` hook:

```sh
cargo deny --workspace --config .github/lint/deny.toml check
```

It checks advisories, bans, licenses and sources for every workspace member, including tests and build and dev dependencies. Install it with `cargo install cargo-deny --locked`.

`.github/lint/deny.toml` allows a fixed list of licenses: MIT, Apache-2.0 (with or without the LLVM exception), BSD-2-Clause, BSD-3-Clause, BSL-1.0 (the Boost Software License), IJG, ISC, Zlib, 0BSD, Unicode-3.0, CDLA-Permissive-2.0 and MPL-2.0. A crate with any other license, or none, fails the check. Git dependencies are allowed only from the DioxusLabs anyrender, taffy and vello repositories, listed in `.github/lint/deny.local.toml`. Before you add a license to the list, read its terms and the crate's license files.

## What the dependency graph contains

The workspace resolves 646 crates. Almost all of them are MIT, Apache-2.0 or a choice between permissive licenses.

| License | In a closed-source app | What you owe when you ship |
| --- | --- | --- |
| MIT, ISC, BSD-2-Clause, BSD-3-Clause, 0BSD | Allowed, no source needed | Keep the copyright and license notices. BSD-3-Clause also bars using the authors' names to promote your app. |
| Apache-2.0 | Allowed, no source needed | Include the license and any NOTICE file, and mark files you changed. It grants patent rights, which end if you sue over patents in the work. |
| BSL-1.0, Zlib | Allowed, no source needed | Keep the notices. Zlib asks you to mark changed source as changed. |
| Unicode-3.0 | Allowed | Keep the copyright and permission notice with the software or its documentation. |
| MPL-2.0 | Allowed | Make the source of the MPL files you ship available, changed or not, and tell users where to get it. |

`OR` in a license field means you pick one. `AND` means all of them apply. A few crates need a second license on top of MIT or Apache-2.0:

- `encoding_rs` also needs BSD-3-Clause, and `unicode-ident` also needs Unicode-3.0.
- The ICU4X crates (`icu_*`, `zerovec`, `yoke`, `tinystr` and others), which Boa uses for `Intl` and text segmentation, are Unicode-3.0 only.
- `tiny-skia`, `alloc-no-stdlib` and `alloc-stdlib` are BSD-3-Clause only.
- `blitz-chrome-devtools-frontend` is BSD-3-Clause AND MIT AND Apache-2.0. It's only built with the `embedded-devtools-frontend` feature of `pageframe-shell`.

`r-efi` offers LGPL-2.1 as one choice among MIT and Apache-2.0, so it doesn't bring in the LGPL. Nothing in the graph requires GPL, AGPL or LGPL.

## MPL-2.0 code

These crates are MPL-2.0 only:

- The Stylo copy in `vendor/stylo`: `stylo`, `stylo_atoms`, `stylo_derive`, `stylo_dom`, `stylo_static_prefs`, `stylo_traits`, `selectors`, `to_shmem` and `to_shmem_derive`. The `servo_arc` and `malloc_size_of` crates in the same folder are MIT OR Apache-2.0.
- From crates.io: `app_units`, `cssparser`, `cssparser-macros`, `dtoa-short` and `uluru`.
- `colored`, which only the WPT runner in `wpt/runner` uses. It isn't part of an app.

`stylo_taffy` offers MPL-2.0 alongside MIT and Apache-2.0, so you can take it under MIT or Apache-2.0.

MPL-2.0 works file by file. Your own files stay under your license, and linking MPL code into your binary, statically or not, doesn't change that. What you must do, under sections 3.1 to 3.3 of the [MPL](https://www.mozilla.org/en-US/MPL/2.0/):

- Make the source of every MPL file in your binary available to the people you give it to, even if you didn't change those files.
- Tell them how to get that source.
- Keep any changes to those files under the MPL.

If you copy MPL code into one of your own files, the MPL applies to that file. Mozilla's [MPL FAQ](https://www.mozilla.org/en-US/MPL/2.0/FAQ/) explains where a covered file ends and your larger work begins.

[NOTICE](../NOTICE) lists the changes to Stylo. When you ship, point users to [`vendor/stylo`](../vendor/stylo) at the exact commit you built from. The [MPL-2.0 text](../vendor/stylo/LICENSE-MPL-2.0) is in that folder.

## Files cargo-deny doesn't see

cargo-deny only reads Cargo metadata. Check these files separately when you ship `pageframe-dom`.

- `crates/pageframe-dom/assets/default.css` comes from Servo and is MPL-2.0, as its header says. The same source rules apply.
- `crates/pageframe-dom/assets/quirks.css` was written for Pageframe from the quirks-mode rules in the HTML Standard and is MIT OR Apache-2.0 like the rest of the repository.
- `crates/pageframe-dom/assets/moz-bullet-font.otf` is a modified Mozilla Bullet 1.0 by Mats Palmgren. Pageframe inherited the OTF from [Blitz at b6bb45c](https://github.com/DioxusLabs/blitz/blob/b6bb45c550a9490afa6c43468c77535c6d76856d/packages/blitz-dom/assets/moz-bullet-font.otf). The OTF has no license notice. The [original Gecko font source](https://github.com/mozilla/gecko-dev/blob/e4d7dd995a62ca5653527792b55c8f6ee6e74b02/layout/style/res/Mozilla_Bullet.bf) had blank license fields. Mozilla [added a SIL Open Font License notice](https://github.com/mozilla/gecko-dev/commit/02e33379c1292f7dcd2b2ff56e01c5461154aaa3) in a later revision. Confirm the license for the bundled OTF before distributing it.

## Before you ship an app

- Generate third-party notices from the crates, features and targets you actually ship. A passing cargo-deny check doesn't produce them.
- Include the license texts and NOTICE content those crates require, including this repository's NOTICE file.
- For the MPL files you ship, publish the exact source you built from and tell users where to find it.
- Check anything Cargo doesn't describe: system libraries, fonts, images and the pages your app loads.
