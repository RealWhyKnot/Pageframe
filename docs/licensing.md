# Licensing

## Shipping an app with Pageframe

Pageframe's own code is MIT OR Apache-2.0. The default build uses no code under GPL, AGPL or LGPL terms. You can ship Pageframe in a permissively licensed app, a proprietary app or a `GPL-3.0-or-later` app such as Backtrack.

1. Include Pageframe's [NOTICE](../NOTICE), the Pageframe license you choose, and the copyright notices and license texts for the dependencies you ship, including their MIT, Apache-2.0 and BSD terms.
2. Generate the Cargo notices from your app's manifest, lockfile, features and targets. Copy Pageframe's [about.toml](../about.toml) into your app, set `targets` to the target triples you ship, and run the command below from the app's Cargo package directory with [cargo-about](https://embarkstudios.github.io/cargo-about/) installed. Pass the same feature flags you use for the release build.
3. In the notices you ship, link to [`vendor/stylo`](../vendor/stylo) and the [Servo stylesheet](../crates/pageframe-dom/assets/default.css) at the exact Pageframe commit or tag you built. Provide the corresponding source for the other MPL-2.0 dependencies you ship. If you changed any MPL files, make your exact versions available under MPL-2.0 and tell recipients where to get them. See the [MPL FAQ](https://www.mozilla.org/en-US/MPL/2.0/FAQ/).

```sh
cargo about generate --locked --fail --format json --output-file third-party-licenses.json
```

Use cargo-about 0.9 or later. It includes local Pageframe and Stylo crates by default.

The JSON includes the collected Cargo license texts. For formatted HTML, pass your [Handlebars template](https://embarkstudios.github.io/cargo-about/cli/generate/index.html) in place of `--format json` and name the output file accordingly. Include required notices for anything you bundle outside Cargo, such as system libraries, fonts and images.

## Maintainer checks

The CI `checks` job and pre-push hook run cargo-deny with all workspace features enabled.

```sh
cargo deny --manifest-path Cargo.toml --workspace --config .github/lint/deny.toml check
```

The [active cargo-deny config](../.github/lint/deny.toml) checks advisories, bans, licenses and sources, including test and build dependencies. Its license allowlist covers MIT, Apache-2.0 (including the LLVM exception), BSD-2-Clause, BSD-3-Clause, BSL-1.0, ISC, Zlib, 0BSD, Unicode-3.0 and MPL-2.0. When updating the license allowlist, review the crate's license files and keep `about.toml` in sync.

Keep approved Git sources in the [local Git source allowlist](../.github/lint/deny.local.toml). `Install-Lint -Force` merges it into `deny.toml` when refreshing the shared lint configs.

In a Cargo license expression, `OR` lets you choose a license and `AND` requires you to comply with each term. For example, `unicode-ident` requires Unicode-3.0 in addition to a choice of MIT or Apache-2.0. `r-efi` offers LGPL-2.1 as an alternative to its permissive licenses. The policy uses a permissive option.

### MPL-2.0 code

- Most crates in [vendor/stylo](../vendor/stylo) are MPL-2.0. `servo_arc` and `malloc_size_of` are MIT OR Apache-2.0. Stylo's license text is in [LICENSE-MPL-2.0](../vendor/stylo/LICENSE-MPL-2.0).
- The registry crates `app_units`, `cssparser`, `cssparser-macros`, `dtoa-short` and `uluru` are MPL-2.0. The WPT runner also uses `colored` under MPL-2.0. `stylo_taffy` offers MIT or Apache-2.0 as well as MPL-2.0.
- MPL-2.0 applies file by file. A file that incorporates MPL code is covered by the MPL. You can distribute a larger work under your app's terms while making the covered files available under MPL-2.0. See [MPL-2.0 sections 3.1 to 3.3](https://www.mozilla.org/en-US/MPL/2.0/).

### Files outside Cargo metadata

- [default.css](../crates/pageframe-dom/assets/default.css), the Servo user-agent stylesheet embedded in `pageframe-dom`, is MPL-2.0. [quirks.css](../crates/pageframe-dom/assets/quirks.css) is MIT OR Apache-2.0.
- Stylo's [vendored Python packages](../vendor/stylo/style/properties/vendored_python) are used during the build. Mako and toml are MIT, and MarkupSafe is BSD-3-Clause. See [NOTICE](../NOTICE) for the attributions and license locations.
- The `pageframe-script` test fixture includes a copy of [Preact](../crates/pageframe-script/tests/preact_app/vendor) under MIT.
