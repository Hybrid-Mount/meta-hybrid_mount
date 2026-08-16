/*
 * Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

import { describe, expect, it } from "vitest";
import enUS from "../../locales/en-US.json";
import { validateLocaleShape } from "./uiStore";

const locales = import.meta.glob<{ default: unknown }>("../../locales/*.json", {
  eager: true,
});

describe("locale contracts", () => {
  for (const [path, locale] of Object.entries(locales)) {
    it(`${path} matches the en-US key shape`, () => {
      expect(() =>
        validateLocaleShape(enUS, locale.default, path),
      ).not.toThrow();
    });
  }
});
