# Security policy

Pageframe is early software that loads HTML, CSS and JavaScript into a desktop app. Pages can run scripts, make network requests and call into the app over IPC.

## Reporting

**Don't open a public issue for security reports.**

Use [GitHub Security Advisories](https://github.com/RealWhyKnot/Pageframe/security/advisories/new). It's a private channel where we can work out a fix and a disclosure timeline.

I try to acknowledge new reports within **7 days** and give a first assessment within **14 days**. There's no bug bounty.

## In scope

- A page loaded over the network reading local files, or loading `file:` URLs it shouldn't reach.
- A page breaking out of the navigation policy, or reaching IPC handlers the app didn't mean to expose to it.
- The DevTools server accepting connections or requests from web pages, or serving files outside its frontend folder.
- Memory safety bugs, crashes or hangs that a page can trigger in the app, including through malformed HTML, CSS, images, fonts or media.
- The HTTP client mishandling responses in a way a server can exploit, such as reading past a body or accepting a truncated one as complete.

## Out of scope

- Apps that widen the defaults, such as a navigation policy that allows any URL, or an IPC handler that acts on requests without checking which page sent them.
- Bugs in upstream projects (Stylo, Boa, Vello and others) that don't affect Pageframe. Report those upstream.
- Rendering differences from Chrome. Those are functional bugs. Open a normal issue.

## Disclosure

I prefer coordinated disclosure: I'll work with the reporter on a fix and a timeline before publishing details. Reporters are credited in the advisory unless they ask not to be named.
