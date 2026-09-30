---
name: vue-testing-best-practices
description: 给 Hybrid Mount WebUI 写或修测试时用：Vitest + Vue Test Utils 的组件测试、异步与 flushPromises、composable 测试、避免只写快照，以及本仓库实际的测试门禁。
whenToUse: 新增 webui/src 下的 vitest 测试、测试因重构或竞态而不稳定、或需要判断某个行为该怎么测时。
license: MIT
author: github.com/vuejs-ai
version: 1.0.0
metadata:
  vendored:
    source: https://github.com/vuejs-ai/skills
    commit: c9d355ff23f654309dd02006be671859df0a134c
    vendored: "2026-09-30"
    upstream-path: skills/vue-testing-best-practices
---

## Hybrid Mount precedence (read before applying any rule)

These rules are generic Vue testing defaults. This repository's own contracts win wherever they differ.

- **The runner is `pnpm test` = `vitest run && vue-tsc -b`** from `webui/`; types are part of the test gate, so a passing Vitest run with failing `vue-tsc` is still red.
- **No Playwright, no E2E, no browser runner.** The WebUI is a WebView front-end for a rooted Android device; the `testing-e2e-playwright-recommended` and `testing-browser-vs-node-runners` references describe an environment this repo does not have. Do not add them.
- **No Pinia.** Any reference telling you to install or hydrate a Pinia store before mounting does not apply; the `testing-pinia-store-setup` reference is kept only because it also covers test-only plugin installation, which `vue-i18n` does need.
- **This is a systems repo.** Most behaviour worth testing lives in Rust (`cargo test --workspace`); do not grow a Vue test suite to compensate for untested Rust, and do not test the `kernelsu.exec` bridge by mocking it into always succeeding.
- **Don't snapshot what a gate can assert.** See `.dsh/skills/hm-verify/SKILL.md` for the full checklist.

Vue.js testing best practices, patterns, and common gotchas.

### Testing
- Setting up test infrastructure for Vue 3 projects → See [testing-vitest-recommended-for-vue](reference/testing-vitest-recommended-for-vue.md)
- Tests keep breaking when refactoring component internals → See [testing-component-blackbox-approach](reference/testing-component-blackbox-approach.md)
- Tests fail intermittently with race conditions → See [testing-async-await-flushpromises](reference/testing-async-await-flushpromises.md)
- Composables using lifecycle hooks or inject fail to test → See [testing-composables-helper-wrapper](reference/testing-composables-helper-wrapper.md)
- Getting "injection Symbol(pinia) not found" errors in tests → See [testing-pinia-store-setup](reference/testing-pinia-store-setup.md)
- Components with async setup won't render in tests → See [testing-suspense-async-components](reference/testing-suspense-async-components.md)
- Snapshot tests keep passing despite broken functionality → See [testing-no-snapshot-only](reference/testing-no-snapshot-only.md)
- Choosing end-to-end testing framework for Vue apps → See [testing-e2e-playwright-recommended](reference/testing-e2e-playwright-recommended.md)
- Tests need to verify computed styles or real DOM events → See [testing-browser-vs-node-runners](reference/testing-browser-vs-node-runners.md)
- Testing components created with defineAsyncComponent fails → See [async-component-testing](reference/async-component-testing.md)
- Teleported modal content can't be found in wrapper queries → See [teleport-testing-complexity](reference/teleport-testing-complexity.md)

## Reference

- [Vue.js Testing Guide](https://vuejs.org/guide/scaling-up/testing)
- [Vue Test Utils](https://test-utils.vuejs.org/)
- [Vitest Documentation](https://vitest.dev/)
- [Playwright Documentation](https://playwright.dev/)
