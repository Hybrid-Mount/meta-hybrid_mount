// SPDX-License-Identifier: Apache-2.0
export const shellEscapeDoubleQuoted = (value: string): string =>
  value.replace(/(["\\$`])/g, "\\$1");
