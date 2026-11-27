# Security policy

Pageframe is early software that loads HTML, CSS and JavaScript into a desktop app. Pages can run scripts, make network requests and call into the app over IPC.

## Reporting

Report security problems privately through [GitHub Security Advisories](https://github.com/RealWhyKnot/Pageframe/security/advisories/new).

I aim to acknowledge reports within 7 days and share an initial assessment within 14 days. Pageframe has no bug bounty.

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

I'll work with you on a fix and a disclosure timeline before publishing details. I'll credit you in the advisory unless you ask to stay anonymous.
