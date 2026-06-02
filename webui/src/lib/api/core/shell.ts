export function shellEscapeDoubleQuoted(value: string): string {
  return value.replace(/(["\\$`])/g, "\\$1");
}
